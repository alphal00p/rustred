//! Immutable solve order for one authenticated generated affine group.
//!
//! LiteRed orders the starts inside one contiguous group from simpler to
//! harder by reversing its descending integral sort.  This certificate makes
//! that order explicit under RustRed's persisted physical-key policy.  It is
//! deliberately not the mutable group database: depth, submitted equations,
//! rejected pivots, consumed targets, `WhenBad` events, and outer group
//! selection all belong to later owners.

use std::cmp::Ordering;
use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::physical_key::{
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA,
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA,
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V3_SCHEMA,
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
    GeneratedAffineResidualGroupPhysicalKeyError, GeneratedAffineResidualGroupPhysicalKeyPreflight,
};
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualCaseAuthoritySourceKind,
    GeneratedAffineResidualCaseInventoryCertificate, GeneratedAffineResidualCaseInventoryError,
    GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::{IntegralFamily, ParametricCoefficientContext};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-solve-plan-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-solve-plan-v2";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-solve-plan-v3";
const TARGET_ORDER_V1_ID: &str = "stable-ascending-physical-key-then-inventory-position-v1";

const INVENTORY_ALLOCATION_COMPARISONS: usize = 1;
const FRAME_REPLAYS: usize = 1;
const GROUP_AUTHENTICATIONS: usize = 1;
const LEGACY_RETAINED_PARENT_REFERENCES: usize = 3;
const DIRECT_RETAINED_PARENT_REFERENCES: usize = 2;
const LIMIT_SCALAR_FIELDS: usize = 25;
const STATS_SCALAR_FIELDS: usize = 29;

const fn solve_plan_schema_for_source(
    source: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V2_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupSolvePlanLimits {
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_inventory_allocation_comparisons: usize,
    pub(crate) max_frame_replays: usize,
    pub(crate) max_group_authentications: usize,
    pub(crate) max_retained_parent_references: usize,
    pub(crate) max_group_cases: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_free_positions: usize,
    pub(crate) max_target_locators: usize,
    pub(crate) max_key_aggregate_preflights: usize,
    pub(crate) max_key_constructions: usize,
    pub(crate) max_key_component_scans: usize,
    pub(crate) max_key_integer_bit_work: usize,
    pub(crate) max_key_prospective_retained_integer_bits: usize,
    pub(crate) max_key_prospective_retained_bytes: usize,
    pub(crate) max_key_observed_retained_integer_bits: usize,
    pub(crate) max_key_observed_retained_bytes: usize,
    pub(crate) max_sort_passes: usize,
    pub(crate) max_sort_comparisons: usize,
    pub(crate) max_sort_comparison_integer_bit_work: usize,
    pub(crate) max_sort_moves: usize,
    pub(crate) max_permutation_validation_scans: usize,
    pub(crate) max_manifest_bytes: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupSolvePlanLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_inventory_allocation_comparisons: INVENTORY_ALLOCATION_COMPARISONS,
            max_frame_replays: FRAME_REPLAYS,
            max_group_authentications: GROUP_AUTHENTICATIONS,
            max_retained_parent_references: LEGACY_RETAINED_PARENT_REFERENCES,
            max_group_cases: 256_000_000,
            max_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_target_locators: 256_000_000,
            max_key_aggregate_preflights: 256_000_000,
            max_key_constructions: 256_000_000,
            max_key_component_scans: LARGE,
            max_key_integer_bit_work: VERY_LARGE,
            max_key_prospective_retained_integer_bits: VERY_LARGE,
            max_key_prospective_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_key_observed_retained_integer_bits: VERY_LARGE,
            max_key_observed_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_sort_passes: 256,
            max_sort_comparisons: LARGE,
            max_sort_comparison_integer_bit_work: VERY_LARGE,
            max_sort_moves: LARGE,
            max_permutation_validation_scans: LARGE,
            max_manifest_bytes: 4 * 1024 * 1024 * 1024,
            max_owner_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_peak_scratch_bytes: 256 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupSolvePlanStats {
    scope_comparison_bytes: usize,
    inventory_allocation_comparisons: usize,
    frame_replays: usize,
    group_authentications: usize,
    retained_parent_references: usize,
    group_cases: usize,
    arity: usize,
    free_positions: usize,
    target_locators: usize,
    key_aggregate_preflights: usize,
    key_constructions: usize,
    key_component_scans: usize,
    key_integer_bit_work: usize,
    key_prospective_retained_integer_bits: usize,
    maximum_key_prospective_retained_integer_bits: usize,
    key_prospective_retained_bytes: usize,
    key_observed_retained_integer_bits: usize,
    key_observed_retained_bytes: usize,
    sort_passes: usize,
    sort_comparisons: usize,
    sort_comparison_integer_bit_work: usize,
    sort_moves: usize,
    permutation_validation_scans: usize,
    manifest_bytes: usize,
    owner_retained_bytes: usize,
    peak_scratch_bytes: usize,
    replay_combined_owner_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupSolvePlanStats {
    stats_getters!(
        scope_comparison_bytes,
        inventory_allocation_comparisons,
        frame_replays,
        group_authentications,
        retained_parent_references,
        group_cases,
        arity,
        free_positions,
        target_locators,
        key_aggregate_preflights,
        key_constructions,
        key_component_scans,
        key_integer_bit_work,
        key_prospective_retained_integer_bits,
        maximum_key_prospective_retained_integer_bits,
        key_prospective_retained_bytes,
        key_observed_retained_integer_bits,
        key_observed_retained_bytes,
        sort_passes,
        sort_comparisons,
        sort_comparison_integer_bit_work,
        sort_moves,
        permutation_validation_scans,
        manifest_bytes,
        owner_retained_bytes,
        peak_scratch_bytes,
        replay_combined_owner_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupSolvePlanReplayLimits {
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_combined_owner_bytes: usize,
    pub(crate) max_payload_comparison_units: usize,
    pub(crate) max_payload_comparison_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupSolvePlanReplayLimits {
    fn default() -> Self {
        Self {
            max_parent_allocation_comparisons: LEGACY_RETAINED_PARENT_REFERENCES,
            max_combined_owner_bytes: 512 * 1024 * 1024 * 1024,
            max_payload_comparison_units: 64_000_000_000,
            max_payload_comparison_bytes: 256 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupSolvePlanError {
    SchemaMismatch,
    ReplayMismatch,
    WrongInventoryAllocation,
    WrongAuthorityAllocation,
    WrongFrameAllocation,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongGroup,
    NonCanonicalGroupAuthority,
    MalformedGroup,
    Inventory,
    Authority,
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

impl GeneratedAffineResidualGroupSolvePlanError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongInventoryAllocation => "WrongInventoryAllocation",
            Self::WrongAuthorityAllocation => "WrongAuthorityAllocation",
            Self::WrongFrameAllocation => "WrongFrameAllocation",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongGroup => "WrongGroup",
            Self::NonCanonicalGroupAuthority => "NonCanonicalGroupAuthority",
            Self::MalformedGroup => "MalformedGroup",
            Self::Inventory => "Inventory",
            Self::Authority => "Authority",
            Self::PhysicalKey => "PhysicalKey",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupSolvePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupSolvePlanError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupSolvePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine group solve-plan {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupSolvePlanError {}

impl From<GeneratedAffineResidualCaseAuthorityError>
    for GeneratedAffineResidualGroupSolvePlanError
{
    fn from(_: GeneratedAffineResidualCaseAuthorityError) -> Self {
        Self::Authority
    }
}

impl From<GeneratedAffineResidualCaseInventoryError>
    for GeneratedAffineResidualGroupSolvePlanError
{
    fn from(_: GeneratedAffineResidualCaseInventoryError) -> Self {
        Self::Inventory
    }
}

impl From<GeneratedAffineResidualGroupPhysicalKeyError>
    for GeneratedAffineResidualGroupSolvePlanError
{
    fn from(error: GeneratedAffineResidualGroupPhysicalKeyError) -> Self {
        match error {
            GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation => {
                Self::WrongAuthorityAllocation
            }
            GeneratedAffineResidualGroupPhysicalKeyError::WrongGroup => Self::WrongGroup,
            GeneratedAffineResidualGroupPhysicalKeyError::WrongArity { .. } => Self::WrongArity,
            _ => Self::PhysicalKey,
        }
    }
}

/// Exact positional locator in the persisted within-group solve order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupSolveTargetLocator {
    solve_ordinal: usize,
    inventory_position: usize,
    case_ordinal: usize,
}

impl GeneratedAffineResidualGroupSolveTargetLocator {
    pub(crate) const fn solve_ordinal(self) -> usize {
        self.solve_ordinal
    }
    pub(crate) const fn inventory_position(self) -> usize {
        self.inventory_position
    }
    pub(crate) const fn case_ordinal(self) -> usize {
        self.case_ordinal
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupSolveTargetLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupSolveTargetLocator")
            .field("solve_ordinal", &self.solve_ordinal)
            .field("private_inventory_position", &"<redacted>")
            .field("private_case_ordinal", &"<redacted>")
            .finish()
    }
}

struct SortEntry {
    inventory_position: usize,
    case_ordinal: usize,
    key: GeneratedAffineResidualGroupPhysicalKey,
}

#[derive(Clone)]
enum GeneratedAffineResidualGroupSolvePlanSource {
    LegacyInventory(Arc<GeneratedAffineResidualCaseInventoryCertificate>),
    DirectFormulaSingleton,
    CommittedExceptionalSingleton,
}

impl GeneratedAffineResidualGroupSolvePlanSource {
    const fn kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        match self {
            Self::LegacyInventory(_) => {
                GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory
            }
            Self::DirectFormulaSingleton => {
                GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            }
            Self::CommittedExceptionalSingleton => {
                GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
            }
        }
    }

    const fn retained_parent_references(&self) -> usize {
        match self {
            Self::LegacyInventory(_) => LEGACY_RETAINED_PARENT_REFERENCES,
            Self::DirectFormulaSingleton | Self::CommittedExceptionalSingleton => {
                DIRECT_RETAINED_PARENT_REFERENCES
            }
        }
    }

    const fn inventory_allocation_comparisons(&self) -> usize {
        match self {
            Self::LegacyInventory(_) => INVENTORY_ALLOCATION_COMPARISONS,
            Self::DirectFormulaSingleton | Self::CommittedExceptionalSingleton => 0,
        }
    }
}

impl fmt::Debug for SortEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SortEntry")
            .field("private_inventory_position", &"<redacted>")
            .field("private_case_ordinal", &"<redacted>")
            .field("private_key", &"<redacted>")
            .finish()
    }
}

/// Immutable, allocation-bound order for one selected geometry group.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupSolvePlan {
    schema: &'static str,
    source: GeneratedAffineResidualGroupSolvePlanSource,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    group_ordinal: usize,
    anchor_case_ordinal: usize,
    free_positions: Arc<Vec<usize>>,
    targets: Arc<Vec<GeneratedAffineResidualGroupSolveTargetLocator>>,
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    stats: GeneratedAffineResidualGroupSolvePlanStats,
    stable_manifest: Arc<String>,
}

impl fmt::Debug for GeneratedAffineResidualGroupSolvePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupSolvePlan")
            .field("schema", &self.schema)
            .field("group_ordinal", &self.group_ordinal)
            .field("anchor_case_ordinal", &self.anchor_case_ordinal)
            .field("arity", &self.physical_frame.arity())
            .field("free_position_count", &self.free_positions.len())
            .field("target_count", &self.targets.len())
            .field("stats", &self.stats)
            .field("private_source", &"<redacted>")
            .field("private_authority", &"<redacted>")
            .field("private_physical_frame", &"<redacted>")
            .field("private_order", &"<redacted>")
            .field("private_manifest", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualGroupSolvePlan {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupSolvePlanLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_unwind_boundary(
                family,
                context,
                inventory,
                authority,
                physical_frame,
                limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    fn try_new_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupSolvePlanLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupSolvePlanError> {
        Self::try_new_for_source_unwind_boundary(
            family,
            context,
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(inventory),
            authority,
            physical_frame,
            limits,
        )
    }

    pub(crate) fn try_new_direct_formula_singleton(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupSolvePlanLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_for_source_unwind_boundary(
                family,
                context,
                GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton,
                authority,
                physical_frame,
                limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    pub(crate) fn try_new_committed_exceptional_singleton(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupSolvePlanLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_for_source_unwind_boundary(
                family,
                context,
                GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton,
                authority,
                physical_frame,
                limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    fn try_new_for_source_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: GeneratedAffineResidualGroupSolvePlanSource,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        limits: GeneratedAffineResidualGroupSolvePlanLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupSolvePlanError> {
        let mut stats = GeneratedAffineResidualGroupSolvePlanStats::default();
        authenticate_parents(
            family,
            context,
            &source,
            &authority,
            &physical_frame,
            limits,
            &mut stats,
        )?;
        let preliminary_shape = GroupShape {
            group_cases: physical_frame.case_ordinals().len(),
            arity: physical_frame.arity(),
            free_positions: physical_frame.stats().free_positions(),
        };
        if authority.case_ordinal() != physical_frame.anchor_case_ordinal() {
            return Err(GeneratedAffineResidualGroupSolvePlanError::NonCanonicalGroupAuthority);
        }
        for (resource, requested, limit) in [
            (
                "group cases",
                preliminary_shape.group_cases,
                limits.max_group_cases,
            ),
            ("ambient arity", preliminary_shape.arity, limits.max_arity),
            (
                "free positions",
                preliminary_shape.free_positions,
                limits.max_free_positions,
            ),
            (
                "target locators",
                preliminary_shape.group_cases,
                limits.max_target_locators,
            ),
            (
                "physical-key aggregate preflights",
                preliminary_shape.group_cases,
                limits.max_key_aggregate_preflights,
            ),
            (
                "physical-key constructions",
                preliminary_shape.group_cases,
                limits.max_key_constructions,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        let group = authority.authenticated_source_neutral_group_view(context)?;
        stats.group_authentications = GROUP_AUTHENTICATIONS;
        let shape = authenticate_group_shape(authority.as_ref(), physical_frame.as_ref(), group)?;
        if shape.group_cases != preliminary_shape.group_cases
            || shape.arity != preliminary_shape.arity
            || shape.free_positions != preliminary_shape.free_positions
        {
            return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
        }
        stats.group_cases = shape.group_cases;
        stats.arity = shape.arity;
        stats.free_positions = shape.free_positions;
        stats.target_locators = shape.group_cases;
        // Admit every statically sized construction buffer before reserving
        // the preflight-token array.  The second admission below adds the
        // data-dependent prospective GMP-backed key payload after all opaque
        // preflights have been censused, but before any key is constructed.
        let static_scratch_bound =
            construction_scratch_bound(shape.group_cases, shape.free_positions, stats)?;
        check_limit(
            "solve-plan peak scratch bytes",
            static_scratch_bound,
            limits.max_peak_scratch_bytes,
        )?;
        stats.peak_scratch_bytes = static_scratch_bound;
        // Complete aggregate admission before retaining the first temporary
        // GMP-backed key. Every opaque token binds this exact frame allocation
        // and exact canonical offset; consuming it cannot substitute either.
        let mut key_preflights =
            try_vec_with_capacity("physical-key preflight tokens", shape.group_cases)?;
        for (position, &case_ordinal) in group.case_ordinals().iter().enumerate() {
            let offset = physical_frame.anchor_offset(position, case_ordinal)?;
            let census = physical_frame.preflight_key_for_physical(offset)?;
            stats.key_aggregate_preflights = bounded_add(
                "physical-key aggregate preflights",
                stats.key_aggregate_preflights,
                1,
                limits.max_key_aggregate_preflights,
            )?;
            stats.key_component_scans = bounded_add(
                "physical-key component scans",
                stats.key_component_scans,
                census.component_scans(),
                limits.max_key_component_scans,
            )?;
            stats.key_integer_bit_work = bounded_add(
                "physical-key integer-bit work",
                stats.key_integer_bit_work,
                census.integer_bit_work(),
                limits.max_key_integer_bit_work,
            )?;
            stats.key_prospective_retained_integer_bits = bounded_add(
                "prospective physical-key retained integer bits",
                stats.key_prospective_retained_integer_bits,
                census.prospective_retained_integer_bits(),
                limits.max_key_prospective_retained_integer_bits,
            )?;
            stats.maximum_key_prospective_retained_integer_bits = stats
                .maximum_key_prospective_retained_integer_bits
                .max(census.prospective_retained_integer_bits());
            stats.key_prospective_retained_bytes = bounded_add(
                "prospective physical-key retained bytes",
                stats.key_prospective_retained_bytes,
                census.prospective_retained_bytes(),
                limits.max_key_prospective_retained_bytes,
            )?;
            key_preflights.push(census);
        }

        let sort_pass_bound = ceil_log2(shape.group_cases);
        check_limit(
            "stable-sort passes",
            sort_pass_bound,
            limits.max_sort_passes,
        )?;
        let comparison_bound = checked_mul(
            "stable-sort comparisons",
            shape.group_cases,
            sort_pass_bound,
        )?;
        check_limit(
            "stable-sort comparisons",
            comparison_bound,
            limits.max_sort_comparisons,
        )?;
        stats.sort_comparison_integer_bit_work = checked_mul(
            "stable-sort comparison integer-bit work",
            checked_mul(
                "stable-sort comparison integer-bit work",
                comparison_bound,
                2,
            )?,
            stats.maximum_key_prospective_retained_integer_bits.max(1),
        )?;
        check_limit(
            "stable-sort comparison integer-bit work",
            stats.sort_comparison_integer_bit_work,
            limits.max_sort_comparison_integer_bit_work,
        )?;
        let setup_move_passes = if shape.group_cases < 2 { 1 } else { 2 };
        let move_bound = checked_mul(
            "stable-sort moves",
            shape.group_cases,
            checked_add("stable-sort moves", sort_pass_bound, setup_move_passes)?,
        )?;
        check_limit("stable-sort moves", move_bound, limits.max_sort_moves)?;
        let scratch_bound =
            construction_scratch_bound(shape.group_cases, shape.free_positions, stats)?;
        check_limit(
            "solve-plan peak scratch bytes",
            scratch_bound,
            limits.max_peak_scratch_bytes,
        )?;
        stats.peak_scratch_bytes = scratch_bound;

        let mut entries = try_vec_with_capacity("physical-key sort entries", shape.group_cases)?;
        for ((inventory_position, &case_ordinal), preflight) in group
            .case_ordinals()
            .iter()
            .enumerate()
            .zip(key_preflights.into_iter())
        {
            let key = physical_frame.key_for_preflight(preflight)?;
            stats.key_constructions = bounded_add(
                "physical-key constructions",
                stats.key_constructions,
                1,
                limits.max_key_constructions,
            )?;
            stats.key_observed_retained_integer_bits = bounded_add(
                "observed physical-key retained integer bits",
                stats.key_observed_retained_integer_bits,
                key.retained_integer_bits(),
                limits.max_key_observed_retained_integer_bits,
            )?;
            stats.key_observed_retained_bytes = bounded_add(
                "observed physical-key retained bytes",
                stats.key_observed_retained_bytes,
                key.retained_bytes(),
                limits.max_key_observed_retained_bytes,
            )?;
            if stats.key_observed_retained_integer_bits
                > stats.key_prospective_retained_integer_bits
                || stats.key_observed_retained_bytes > stats.key_prospective_retained_bytes
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch);
            }
            entries.push(SortEntry {
                inventory_position,
                case_ordinal,
                key,
            });
        }
        let ordered_positions = stable_merge_sort_positions(&entries, limits, &mut stats)?;
        let mut targets = try_vec_with_capacity("solve target locators", shape.group_cases)?;
        for (solve_ordinal, &entry_position) in ordered_positions.iter().enumerate() {
            let entry = entries
                .get(entry_position)
                .ok_or(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup)?;
            targets.push(GeneratedAffineResidualGroupSolveTargetLocator {
                solve_ordinal,
                inventory_position: entry.inventory_position,
                case_ordinal: entry.case_ordinal,
            });
        }
        drop(ordered_positions);
        drop(entries);
        validate_target_permutation(group, &targets, limits, &mut stats)?;
        let free_positions = copy_usizes(group.free_positions(), "solve-plan free positions")?;

        let manifest_bytes = manifest_exact_bytes(
            source.kind(),
            physical_frame.as_ref(),
            group,
            &free_positions,
            &targets,
            limits,
        )?;
        check_limit(
            "solve-plan manifest bytes",
            manifest_bytes,
            limits.max_manifest_bytes,
        )?;
        let prospective_owner_retained_bytes =
            prospective_owner_retained_bytes(&free_positions, &targets, manifest_bytes)?;
        check_limit(
            "solve-plan owner retained bytes",
            prospective_owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        let prospective_manifest_peak = checked_add(
            "solve-plan peak scratch bytes",
            prospective_owner_retained_bytes,
            manifest_bytes,
        )?;
        check_limit(
            "solve-plan peak scratch bytes",
            stats.peak_scratch_bytes.max(prospective_manifest_peak),
            limits.max_peak_scratch_bytes,
        )?;
        let stable_manifest = render_manifest(
            source.kind(),
            physical_frame.as_ref(),
            group,
            &free_positions,
            &targets,
            limits,
            manifest_bytes,
        )?;
        stats.manifest_bytes = manifest_bytes;
        let owner_retained_bytes =
            owner_retained_bytes(&free_positions, &targets, &stable_manifest)?;
        check_limit(
            "solve-plan owner retained bytes",
            owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        stats.owner_retained_bytes = owner_retained_bytes;
        stats.peak_scratch_bytes = stats.peak_scratch_bytes.max(checked_add(
            "solve-plan peak scratch bytes",
            owner_retained_bytes,
            manifest_bytes,
        )?);
        check_limit(
            "solve-plan peak scratch bytes",
            stats.peak_scratch_bytes,
            limits.max_peak_scratch_bytes,
        )?;
        stats.replay_combined_owner_bytes = checked_add(
            "solve-plan replay combined owner bytes",
            owner_retained_bytes,
            stats.peak_scratch_bytes,
        )?;
        let payload = payload_census(
            &free_positions,
            &targets,
            &stable_manifest,
            stats.retained_parent_references,
        )?;
        stats.payload_comparison_units = payload.units;
        stats.payload_comparison_bytes = payload.bytes;
        let group_ordinal = group.ordinal();
        let anchor_case_ordinal = group.anchor_case_ordinal();
        Ok(Self {
            schema: solve_plan_schema_for_source(source.kind()),
            source,
            authority,
            physical_frame,
            group_ordinal,
            anchor_case_ordinal,
            free_positions: Arc::new(free_positions),
            targets: Arc::new(targets),
            limits,
            stats,
            stable_manifest: Arc::new(stable_manifest),
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.source.kind()
    }

    /// Replay the exact source allocation already sealed into this plan.
    ///
    /// This source-neutral dispatch is intentionally allocation-bound: the
    /// legacy arm presents the retained inventory while the direct arm
    /// presents the retained singleton authority and frame.  Callers cannot
    /// use it to substitute an independently compiled, payload-equal source.
    pub(crate) fn replay_retained_source(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        replay_limits: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    ) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
        match &self.source {
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(inventory) => self.replay(
                family,
                context,
                inventory,
                &self.authority,
                &self.physical_frame,
                replay_limits,
            ),
            GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton => self
                .replay_direct_formula_singleton(
                    family,
                    context,
                    &self.authority,
                    &self.physical_frame,
                    replay_limits,
                ),
            GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => self
                .replay_committed_exceptional_singleton(
                    family,
                    context,
                    &self.authority,
                    &self.physical_frame,
                    replay_limits,
                ),
        }
    }
    pub(crate) const fn inventory(
        &self,
    ) -> Option<&Arc<GeneratedAffineResidualCaseInventoryCertificate>> {
        match &self.source {
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(inventory) => {
                Some(inventory)
            }
            GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton
            | GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => None,
        }
    }
    pub(crate) const fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        &self.authority
    }
    pub(crate) const fn physical_frame(&self) -> &Arc<GeneratedAffineResidualGroupPhysicalFrame> {
        &self.physical_frame
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn anchor_case_ordinal(&self) -> usize {
        self.anchor_case_ordinal
    }
    pub(crate) fn free_positions(&self) -> &[usize] {
        self.free_positions.as_slice()
    }
    pub(crate) fn targets(&self) -> &[GeneratedAffineResidualGroupSolveTargetLocator] {
        self.targets.as_slice()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupSolvePlanLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupSolvePlanStats {
        self.stats
    }
    pub(crate) fn stable_manifest(&self) -> &str {
        self.stable_manifest.as_str()
    }
    pub(crate) fn same_parent_allocations(
        &self,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> bool {
        matches!(
            &self.source,
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(retained)
                if Arc::ptr_eq(retained, inventory)
        ) && Arc::ptr_eq(&self.authority, authority)
            && Arc::ptr_eq(&self.physical_frame, physical_frame)
    }

    pub(crate) fn same_direct_parent_allocations(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> bool {
        matches!(
            self.source,
            GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton
        ) && Arc::ptr_eq(&self.authority, authority)
            && Arc::ptr_eq(&self.physical_frame, physical_frame)
    }

    pub(crate) fn same_committed_exceptional_parent_allocations(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> bool {
        matches!(
            self.source,
            GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton
        ) && Arc::ptr_eq(&self.authority, authority)
            && Arc::ptr_eq(&self.physical_frame, physical_frame)
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        replay_limits: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    ) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA {
                return Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch);
            }
            check_limit(
                "solve-plan parent allocation comparisons",
                LEGACY_RETAINED_PARENT_REFERENCES,
                replay_limits.max_parent_allocation_comparisons,
            )?;
            if !matches!(
                &self.source,
                GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(retained)
                    if Arc::ptr_eq(retained, inventory)
            ) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation);
            }
            if !Arc::ptr_eq(&self.authority, authority) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongAuthorityAllocation);
            }
            if !Arc::ptr_eq(&self.physical_frame, physical_frame) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongFrameAllocation);
            }
            check_limit(
                "solve-plan replay combined owner bytes",
                self.stats.replay_combined_owner_bytes,
                replay_limits.max_combined_owner_bytes,
            )?;
            let payload = payload_census(
                self.free_positions.as_ref(),
                self.targets.as_ref(),
                self.stable_manifest.as_ref(),
                LEGACY_RETAINED_PARENT_REFERENCES,
            )?;
            check_limit(
                "solve-plan payload comparison units",
                payload.units,
                replay_limits.max_payload_comparison_units,
            )?;
            check_limit(
                "solve-plan payload comparison bytes",
                payload.bytes,
                replay_limits.max_payload_comparison_bytes,
            )?;
            let rebuilt = Self::try_new_unwind_boundary(
                family,
                context,
                Arc::clone(inventory),
                Arc::clone(authority),
                Arc::clone(physical_frame),
                self.limits,
            )?;
            if self.payload_eq(&rebuilt) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    pub(crate) fn replay_direct_formula_singleton(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        replay_limits: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    ) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V2_SCHEMA
                || !matches!(
                    self.source,
                    GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton
                )
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch);
            }
            check_limit(
                "solve-plan parent allocation comparisons",
                DIRECT_RETAINED_PARENT_REFERENCES,
                replay_limits.max_parent_allocation_comparisons,
            )?;
            if !Arc::ptr_eq(&self.authority, authority) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongAuthorityAllocation);
            }
            if !Arc::ptr_eq(&self.physical_frame, physical_frame) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongFrameAllocation);
            }
            check_limit(
                "solve-plan replay combined owner bytes",
                self.stats.replay_combined_owner_bytes,
                replay_limits.max_combined_owner_bytes,
            )?;
            let payload = payload_census(
                self.free_positions.as_ref(),
                self.targets.as_ref(),
                self.stable_manifest.as_ref(),
                DIRECT_RETAINED_PARENT_REFERENCES,
            )?;
            check_limit(
                "solve-plan payload comparison units",
                payload.units,
                replay_limits.max_payload_comparison_units,
            )?;
            check_limit(
                "solve-plan payload comparison bytes",
                payload.bytes,
                replay_limits.max_payload_comparison_bytes,
            )?;
            let rebuilt = Self::try_new_for_source_unwind_boundary(
                family,
                context,
                GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton,
                Arc::clone(authority),
                Arc::clone(physical_frame),
                self.limits,
            )?;
            if self.payload_eq(&rebuilt) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    pub(crate) fn replay_committed_exceptional_singleton(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
        replay_limits: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    ) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA
                || !matches!(
                    self.source,
                    GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton
                )
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch);
            }
            check_limit(
                "solve-plan parent allocation comparisons",
                DIRECT_RETAINED_PARENT_REFERENCES,
                replay_limits.max_parent_allocation_comparisons,
            )?;
            if !Arc::ptr_eq(&self.authority, authority) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongAuthorityAllocation);
            }
            if !Arc::ptr_eq(&self.physical_frame, physical_frame) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongFrameAllocation);
            }
            check_limit(
                "solve-plan replay combined owner bytes",
                self.stats.replay_combined_owner_bytes,
                replay_limits.max_combined_owner_bytes,
            )?;
            let payload = payload_census(
                self.free_positions.as_ref(),
                self.targets.as_ref(),
                self.stable_manifest.as_ref(),
                DIRECT_RETAINED_PARENT_REFERENCES,
            )?;
            check_limit(
                "solve-plan payload comparison units",
                payload.units,
                replay_limits.max_payload_comparison_units,
            )?;
            check_limit(
                "solve-plan payload comparison bytes",
                payload.bytes,
                replay_limits.max_payload_comparison_bytes,
            )?;
            let rebuilt = Self::try_new_for_source_unwind_boundary(
                family,
                context,
                GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton,
                Arc::clone(authority),
                Arc::clone(physical_frame),
                self.limits,
            )?;
            if self.payload_eq(&rebuilt) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::SymbolicaPanic)?
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && same_solve_plan_source_allocation(&self.source, &other.source)
            && Arc::ptr_eq(&self.authority, &other.authority)
            && Arc::ptr_eq(&self.physical_frame, &other.physical_frame)
            && self.group_ordinal == other.group_ordinal
            && self.anchor_case_ordinal == other.anchor_case_ordinal
            && self.free_positions == other.free_positions
            && self.targets == other.targets
            && self.limits == other.limits
            && self.stats == other.stats
            && self.stable_manifest == other.stable_manifest
    }
}

#[derive(Clone, Copy)]
struct GroupShape {
    group_cases: usize,
    arity: usize,
    free_positions: usize,
}

fn same_solve_plan_source_allocation(
    left: &GeneratedAffineResidualGroupSolvePlanSource,
    right: &GeneratedAffineResidualGroupSolvePlanSource,
) -> bool {
    match (left, right) {
        (
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(left),
            GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(right),
        ) => Arc::ptr_eq(left, right),
        (
            GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton,
            GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton,
        ) => true,
        (
            GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton,
            GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton,
        ) => true,
        _ => false,
    }
}

fn authenticate_parents(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: &GeneratedAffineResidualGroupSolvePlanSource,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    physical_frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    stats: &mut GeneratedAffineResidualGroupSolvePlanStats,
) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
    let inventory_allocation_comparisons = source.inventory_allocation_comparisons();
    let retained_parent_references = source.retained_parent_references();
    for (resource, requested, limit) in [
        (
            "inventory allocation comparisons",
            inventory_allocation_comparisons,
            limits.max_inventory_allocation_comparisons,
        ),
        (
            "physical-frame replays",
            FRAME_REPLAYS,
            limits.max_frame_replays,
        ),
        (
            "group authentications",
            GROUP_AUTHENTICATIONS,
            limits.max_group_authentications,
        ),
        (
            "retained parent references",
            retained_parent_references,
            limits.max_retained_parent_references,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    let scope_comparison_bytes = match source {
        GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(inventory) => checked_sum(
            "solve-plan scope comparison bytes",
            [
                family.fingerprint_ref().len(),
                inventory.family_fingerprint().len(),
                context.fingerprint().len(),
                inventory.context_fingerprint().len(),
                authority.family_fingerprint().len(),
                authority.context_fingerprint().len(),
            ],
        )?,
        GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton
        | GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => {
            checked_sum(
                "solve-plan scope comparison bytes",
                [
                    family.fingerprint_ref().len(),
                    authority.family_fingerprint().len(),
                    context.fingerprint().len(),
                    authority.context_fingerprint().len(),
                ],
            )?
        }
    };
    check_limit(
        "solve-plan scope comparison bytes",
        scope_comparison_bytes,
        limits.max_scope_comparison_bytes,
    )?;
    if family.fingerprint_ref() != authority.family_fingerprint() {
        return Err(GeneratedAffineResidualGroupSolvePlanError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint() {
        return Err(GeneratedAffineResidualGroupSolvePlanError::WrongContext);
    }
    if context.index_count() != authority.arity() {
        return Err(GeneratedAffineResidualGroupSolvePlanError::WrongArity);
    }
    match source {
        GeneratedAffineResidualGroupSolvePlanSource::LegacyInventory(inventory) => {
            if family.fingerprint_ref() != inventory.family_fingerprint() {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongFamily);
            }
            if context.fingerprint() != inventory.context_fingerprint() {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongContext);
            }
            if context.index_count() != inventory.arity() {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongArity);
            }
            if !authority.same_inventory_allocation(inventory) {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation);
            }
        }
        GeneratedAffineResidualGroupSolvePlanSource::DirectFormulaSingleton => {
            if authority.source_kind()
                != GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation);
            }
        }
        GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => {
            if authority.source_kind()
                != GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation);
            }
        }
    }
    let expected_frame_schema = match source.kind() {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V3_SCHEMA
        }
    };
    if authority.source_kind() != source.kind() || physical_frame.schema() != expected_frame_schema
    {
        return Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch);
    }
    physical_frame.replay(family, context, authority)?;
    stats.scope_comparison_bytes = scope_comparison_bytes;
    stats.inventory_allocation_comparisons = inventory_allocation_comparisons;
    stats.frame_replays = FRAME_REPLAYS;
    stats.retained_parent_references = retained_parent_references;
    Ok(())
}

fn authenticate_group_shape(
    authority: &GeneratedAffineResidualCaseAuthority,
    physical_frame: &GeneratedAffineResidualGroupPhysicalFrame,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<GroupShape, GeneratedAffineResidualGroupSolvePlanError> {
    if group.ordinal() != authority.group_ordinal()
        || physical_frame.group_ordinal() != authority.group_ordinal()
    {
        return Err(GeneratedAffineResidualGroupSolvePlanError::WrongGroup);
    }
    if authority.case_ordinal() != group.anchor_case_ordinal() {
        return Err(GeneratedAffineResidualGroupSolvePlanError::NonCanonicalGroupAuthority);
    }
    if physical_frame.anchor_case_ordinal() != group.anchor_case_ordinal()
        || physical_frame.case_ordinals() != group.case_ordinals()
        || group.case_ordinals().is_empty()
        || group.case_ordinals().first().copied() != Some(group.anchor_case_ordinal())
    {
        return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
    }
    if group.ambient_arity() != authority.arity()
        || physical_frame.arity() != authority.arity()
        || group
            .free_positions()
            .iter()
            .any(|&position| position >= authority.arity())
        || group
            .free_positions()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
    }
    Ok(GroupShape {
        group_cases: group.case_ordinals().len(),
        arity: authority.arity(),
        free_positions: group.free_positions().len(),
    })
}

fn stable_merge_sort_positions(
    entries: &[SortEntry],
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    stats: &mut GeneratedAffineResidualGroupSolvePlanStats,
) -> Result<Vec<usize>, GeneratedAffineResidualGroupSolvePlanError> {
    stable_merge_sort_positions_by(
        entries.len(),
        |left, right| {
            let left = entries
                .get(left)
                .ok_or(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup)?;
            let right = entries
                .get(right)
                .ok_or(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup)?;
            Ok(compare_sort_entries(left, right))
        },
        limits,
        stats,
    )
}

fn stable_merge_sort_positions_by<F>(
    count: usize,
    mut compare: F,
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    stats: &mut GeneratedAffineResidualGroupSolvePlanStats,
) -> Result<Vec<usize>, GeneratedAffineResidualGroupSolvePlanError>
where
    F: FnMut(usize, usize) -> Result<Ordering, GeneratedAffineResidualGroupSolvePlanError>,
{
    let mut current = try_vec_with_capacity("stable-sort current positions", count)?;
    for position in 0..count {
        current.push(position);
        stats.sort_moves = bounded_add(
            "stable-sort moves",
            stats.sort_moves,
            1,
            limits.max_sort_moves,
        )?;
    }
    if count < 2 {
        return Ok(current);
    }
    let mut scratch = try_vec_with_capacity("stable-sort scratch positions", count)?;
    scratch.resize(count, 0);
    stats.sort_moves = bounded_add(
        "stable-sort moves",
        stats.sort_moves,
        count,
        limits.max_sort_moves,
    )?;
    let mut width = 1usize;
    while width < count {
        stats.sort_passes = bounded_add(
            "stable-sort passes",
            stats.sort_passes,
            1,
            limits.max_sort_passes,
        )?;
        let step = checked_mul("stable-sort run width", width, 2)?;
        let mut run = 0usize;
        while run < count {
            let middle = run.saturating_add(width).min(count);
            let end = run.saturating_add(step).min(count);
            let mut left = run;
            let mut right = middle;
            for output in run..end {
                let choose_left = if left < middle && right < end {
                    stats.sort_comparisons = bounded_add(
                        "stable-sort comparisons",
                        stats.sort_comparisons,
                        1,
                        limits.max_sort_comparisons,
                    )?;
                    compare(current[left], current[right])? != Ordering::Greater
                } else {
                    left < middle
                };
                scratch[output] = if choose_left {
                    let value = current[left];
                    left += 1;
                    value
                } else {
                    let value = current[right];
                    right += 1;
                    value
                };
                stats.sort_moves = bounded_add(
                    "stable-sort moves",
                    stats.sort_moves,
                    1,
                    limits.max_sort_moves,
                )?;
            }
            run = run.saturating_add(step);
        }
        std::mem::swap(&mut current, &mut scratch);
        width = step;
    }
    Ok(current)
}

fn compare_sort_entries(left: &SortEntry, right: &SortEntry) -> Ordering {
    left.key
        .cmp(&right.key)
        .then_with(|| left.inventory_position.cmp(&right.inventory_position))
}

fn validate_target_permutation(
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    targets: &[GeneratedAffineResidualGroupSolveTargetLocator],
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    stats: &mut GeneratedAffineResidualGroupSolvePlanStats,
) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
    if targets.len() != group.case_ordinals().len() {
        return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
    }
    let scans = checked_mul("permutation validation scans", targets.len(), 2)?;
    check_limit(
        "permutation validation scans",
        scans,
        limits.max_permutation_validation_scans,
    )?;
    let mut seen = try_vec_with_capacity("permutation validation flags", targets.len())?;
    seen.resize(targets.len(), false);
    for (solve_ordinal, target) in targets.iter().enumerate() {
        if target.solve_ordinal != solve_ordinal
            || target.inventory_position >= targets.len()
            || seen[target.inventory_position]
            || group
                .case_ordinals()
                .get(target.inventory_position)
                .copied()
                != Some(target.case_ordinal)
        {
            return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
        }
        seen[target.inventory_position] = true;
    }
    if seen.iter().any(|present| !present) {
        return Err(GeneratedAffineResidualGroupSolvePlanError::MalformedGroup);
    }
    stats.permutation_validation_scans = scans;
    Ok(())
}

fn construction_scratch_bound(
    group_cases: usize,
    free_positions: usize,
    stats: GeneratedAffineResidualGroupSolvePlanStats,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    checked_sum(
        "solve-plan peak scratch bytes",
        [
            stats.key_prospective_retained_bytes,
            checked_mul(
                "solve-plan peak scratch bytes",
                group_cases,
                size_of::<GeneratedAffineResidualGroupPhysicalKeyPreflight>(),
            )?,
            checked_mul(
                "solve-plan peak scratch bytes",
                group_cases,
                checked_mul("solve-plan peak scratch bytes", 6, size_of::<usize>())?,
            )?,
            checked_mul(
                "solve-plan peak scratch bytes",
                free_positions,
                size_of::<usize>(),
            )?,
            checked_mul("solve-plan peak scratch bytes", 7, size_of::<Vec<usize>>())?,
        ],
    )
}

#[derive(Clone, Copy)]
struct PayloadCensus {
    units: usize,
    bytes: usize,
}

fn payload_census(
    free_positions: &Vec<usize>,
    targets: &Vec<GeneratedAffineResidualGroupSolveTargetLocator>,
    manifest: &String,
    retained_parent_references: usize,
) -> Result<PayloadCensus, GeneratedAffineResidualGroupSolvePlanError> {
    let payload_fixed_scalar_comparisons = checked_sum(
        "solve-plan payload comparison units",
        [
            1,
            retained_parent_references,
            2,
            LIMIT_SCALAR_FIELDS,
            STATS_SCALAR_FIELDS,
            3,
        ],
    )?;
    Ok(PayloadCensus {
        units: checked_sum(
            "solve-plan payload comparison units",
            [
                payload_fixed_scalar_comparisons,
                free_positions.len(),
                checked_mul("solve-plan payload comparison units", targets.len(), 3)?,
                manifest.len(),
            ],
        )?,
        bytes: checked_sum(
            "solve-plan payload comparison bytes",
            [
                size_of::<GeneratedAffineResidualGroupSolvePlan>(),
                checked_mul(
                    "solve-plan payload comparison bytes",
                    free_positions.len(),
                    size_of::<usize>(),
                )?,
                checked_mul(
                    "solve-plan payload comparison bytes",
                    targets.len(),
                    size_of::<GeneratedAffineResidualGroupSolveTargetLocator>(),
                )?,
                manifest.len(),
            ],
        )?,
    })
}

fn owner_retained_bytes(
    free_positions: &Vec<usize>,
    targets: &Vec<GeneratedAffineResidualGroupSolveTargetLocator>,
    manifest: &String,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    checked_sum(
        "solve-plan owner retained bytes",
        [
            size_of::<GeneratedAffineResidualGroupSolvePlan>(),
            arc_vec_retained_bytes(free_positions)?,
            arc_vec_retained_bytes(targets)?,
            checked_add(
                "solve-plan owner retained bytes",
                arc_payload_control_and_padding_byte_bound::<String>()?,
                manifest.capacity(),
            )?,
        ],
    )
}

fn prospective_owner_retained_bytes(
    free_positions: &Vec<usize>,
    targets: &Vec<GeneratedAffineResidualGroupSolveTargetLocator>,
    manifest_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    checked_sum(
        "solve-plan prospective owner retained bytes",
        [
            size_of::<GeneratedAffineResidualGroupSolvePlan>(),
            arc_vec_retained_bytes(free_positions)?,
            arc_vec_retained_bytes(targets)?,
            checked_add(
                "solve-plan prospective owner retained bytes",
                arc_payload_control_and_padding_byte_bound::<String>()?,
                manifest_bytes,
            )?,
        ],
    )
}

fn manifest_exact_bytes(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    physical_frame: &GeneratedAffineResidualGroupPhysicalFrame,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    free_positions: &[usize],
    targets: &[GeneratedAffineResidualGroupSolveTargetLocator],
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    let mut counter = CountingWriter::new(limits.max_manifest_bytes);
    if write_manifest(
        &mut counter,
        source_kind,
        physical_frame,
        group,
        free_positions,
        targets,
        limits,
    )
    .is_err()
    {
        return Err(counter.error("solve-plan manifest bytes"));
    }
    Ok(counter.bytes)
}

fn render_manifest(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    physical_frame: &GeneratedAffineResidualGroupPhysicalFrame,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    free_positions: &[usize],
    targets: &[GeneratedAffineResidualGroupSolveTargetLocator],
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
    exact_bytes: usize,
) -> Result<String, GeneratedAffineResidualGroupSolvePlanError> {
    let mut output = String::new();
    output.try_reserve_exact(exact_bytes).map_err(|_| {
        GeneratedAffineResidualGroupSolvePlanError::AllocationFailure {
            resource: "solve-plan manifest bytes",
        }
    })?;
    write_manifest(
        &mut output,
        source_kind,
        physical_frame,
        group,
        free_positions,
        targets,
        limits,
    )
    .map_err(
        |_| GeneratedAffineResidualGroupSolvePlanError::AllocationFailure {
            resource: "solve-plan manifest bytes",
        },
    )?;
    if output.len() != exact_bytes {
        return Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch);
    }
    Ok(output)
}

fn write_manifest(
    output: &mut impl fmt::Write,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    physical_frame: &GeneratedAffineResidualGroupPhysicalFrame,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    free_positions: &[usize],
    targets: &[GeneratedAffineResidualGroupSolveTargetLocator],
    limits: GeneratedAffineResidualGroupSolvePlanLimits,
) -> fmt::Result {
    output.write_str(solve_plan_schema_for_source(source_kind))?;
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {}
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            output.write_str("|source=direct-formula-singleton")?;
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            output.write_str("|source=committed-exceptional-singleton")?;
        }
    }
    write!(
        output,
        "|target-order={TARGET_ORDER_V1_ID}|frame-bytes={}:{}|group={}|anchor-case={}|arity={}|free=[",
        physical_frame.stable_manifest().len(),
        physical_frame.stable_manifest(),
        group.ordinal(),
        group.anchor_case_ordinal(),
        group.ambient_arity(),
    )?;
    write_usizes(output, free_positions)?;
    output.write_str("]|targets=[")?;
    for (position, target) in targets.iter().enumerate() {
        if position != 0 {
            output.write_char(';')?;
        }
        write!(
            output,
            "{}:{}:{}",
            target.solve_ordinal, target.inventory_position, target.case_ordinal
        )?;
    }
    output.write_str("]|limits=")?;
    write_fixed_width_usizes(
        output,
        &[
            limits.max_scope_comparison_bytes,
            limits.max_inventory_allocation_comparisons,
            limits.max_frame_replays,
            limits.max_group_authentications,
            limits.max_retained_parent_references,
            limits.max_group_cases,
            limits.max_arity,
            limits.max_free_positions,
            limits.max_target_locators,
            limits.max_key_aggregate_preflights,
            limits.max_key_constructions,
            limits.max_key_component_scans,
            limits.max_key_integer_bit_work,
            limits.max_key_prospective_retained_integer_bits,
            limits.max_key_prospective_retained_bytes,
            limits.max_key_observed_retained_integer_bits,
            limits.max_key_observed_retained_bytes,
            limits.max_sort_passes,
            limits.max_sort_comparisons,
            limits.max_sort_comparison_integer_bit_work,
            limits.max_sort_moves,
            limits.max_permutation_validation_scans,
            limits.max_manifest_bytes,
            limits.max_owner_retained_bytes,
            limits.max_peak_scratch_bytes,
        ],
    )
}

fn write_fixed_width_usizes(output: &mut impl fmt::Write, values: &[usize]) -> fmt::Result {
    const HEX_DIGITS: usize = size_of::<usize>() * 2;
    for (position, value) in values.iter().enumerate() {
        if position != 0 {
            output.write_char(',')?;
        }
        write!(output, "{value:0HEX_DIGITS$x}")?;
    }
    Ok(())
}

fn write_usizes(output: &mut impl fmt::Write, values: &[usize]) -> fmt::Result {
    for (position, value) in values.iter().enumerate() {
        if position != 0 {
            output.write_char(',')?;
        }
        write!(output, "{value}")?;
    }
    Ok(())
}

struct CountingWriter {
    bytes: usize,
    limit: usize,
    overflowed: bool,
}

impl CountingWriter {
    const fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            overflowed: false,
        }
    }

    fn error(&self, resource: &'static str) -> GeneratedAffineResidualGroupSolvePlanError {
        if self.overflowed {
            GeneratedAffineResidualGroupSolvePlanError::ResourceCountOverflow { resource }
        } else {
            GeneratedAffineResidualGroupSolvePlanError::ResourceLimit {
                resource,
                requested: self.bytes.max(self.limit.saturating_add(1)),
                limit: self.limit,
            }
        }
    }
}

impl fmt::Write for CountingWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(next) = self.bytes.checked_add(value.len()) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        self.bytes = next;
        if next > self.limit {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn copy_usizes(
    source: &[usize],
    resource: &'static str,
) -> Result<Vec<usize>, GeneratedAffineResidualGroupSolvePlanError> {
    let mut output = try_vec_with_capacity(resource, source.len())?;
    output.extend_from_slice(source);
    Ok(output)
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupSolvePlanError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| GeneratedAffineResidualGroupSolvePlanError::AllocationFailure { resource })?;
    Ok(output)
}

fn arc_vec_retained_bytes<T>(
    values: &Vec<T>,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    checked_add(
        "solve-plan owner retained bytes",
        arc_payload_control_and_padding_byte_bound::<Vec<T>>()?,
        checked_mul(
            "solve-plan owner retained bytes",
            values.capacity(),
            size_of::<T>(),
        )?,
    )
}

fn arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    let control = checked_mul("solve-plan Arc payload bytes", 2, size_of::<AtomicUsize>())?;
    checked_sum(
        "solve-plan Arc payload bytes",
        [control, align_of::<T>().saturating_sub(1), size_of::<T>()],
    )
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupSolvePlanError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupSolvePlanError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupSolvePlanError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
    if requested > limit {
        Err(GeneratedAffineResidualGroupSolvePlanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Weak};

    use symbolica::domains::integer::MultiPrecisionInteger;
    use symbolica::prelude::Integer;

    use super::super::physical_key::GeneratedAffineResidualGroupPhysicalKeyLimits;
    use super::super::targets::{
        GeneratedAffineResidualGroupExactTargetCatalog,
        GeneratedAffineResidualGroupExactTargetCatalogLimits,
    };
    use super::*;
    use crate::affine_parametric_ordering::integer_magnitude_bits;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::parametric_sector_formula_affine_terminal::{
        ParametricSectorFormulaAffineTerminalCertificate,
        ParametricSectorFormulaAffineTerminalCompiler, ParametricSectorFormulaAffineTerminalLimits,
    };
    use crate::parametric_sector_formula_residual::{
        ParametricSectorFormulaResidualCursor, ParametricSectorFormulaResidualLimits,
        ParametricSectorFormulaResidualRequest,
    };
    use crate::parametric_sector_normalized_source::{
        ParametricSectorNormalizedCoverageSourceCompiler,
        ParametricSectorNormalizedCoverageSourceLimits,
    };
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits, GeneratedAffineResidualCaseInventoryCompiler,
        GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

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

    fn direct_actionable_terminal(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorFormulaAffineTerminalCertificate>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        discovery_limits
            .coverage
            .max_materialized_product_zero_support_terms = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let compilations = discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect();
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                compilations,
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let path = Arc::new(cursor.next_path().unwrap().unwrap());
        let terminal = Arc::new(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                path,
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap(),
        );
        assert!(terminal.geometry().is_some());
        (family, context, terminal)
    }

    fn direct_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorFormulaAffineTerminalCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) {
        let (family, context, terminal) = direct_actionable_terminal(name);
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new_direct_formula_singleton(
                &family,
                &context,
                Arc::clone(&terminal),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        (family, context, terminal, authority, frame)
    }

    fn selected_fixture(
        name: &str,
        singleton: bool,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) {
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
        let group_ordinal = if singleton {
            (0..inventory.group_count())
                .find(|&ordinal| {
                    inventory
                        .authenticated_group_view(&context, ordinal)
                        .unwrap()
                        .case_ordinals()
                        .len()
                        == 1
                })
                .expect("the natural fixture must contain a singleton group")
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
        if singleton {
            assert_eq!(group.case_ordinals().len(), 1);
        } else {
            assert!(group.case_ordinals().len() > 1);
        }
        let authority = Arc::new(
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
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory, authority, frame)
    }

    fn fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) {
        selected_fixture(name, false)
    }

    fn singleton_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
        Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) {
        selected_fixture(name, true)
    }

    fn exact_plan(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        frame: &Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    ) -> (
        GeneratedAffineResidualGroupSolvePlanLimits,
        GeneratedAffineResidualGroupSolvePlan,
    ) {
        let mut limits = GeneratedAffineResidualGroupSolvePlanLimits::default();
        for _ in 0..32 {
            let plan = GeneratedAffineResidualGroupSolvePlan::try_new(
                family,
                context,
                Arc::clone(inventory),
                Arc::clone(authority),
                Arc::clone(frame),
                limits,
            )
            .unwrap();
            let stats = plan.stats();
            let mut next = limits;
            next.max_scope_comparison_bytes = stats.scope_comparison_bytes();
            next.max_inventory_allocation_comparisons = stats.inventory_allocation_comparisons();
            next.max_frame_replays = stats.frame_replays();
            next.max_group_authentications = stats.group_authentications();
            next.max_retained_parent_references = stats.retained_parent_references();
            next.max_group_cases = stats.group_cases();
            next.max_arity = stats.arity();
            next.max_free_positions = stats.free_positions();
            next.max_target_locators = stats.target_locators();
            next.max_key_aggregate_preflights = stats.key_aggregate_preflights();
            next.max_key_constructions = stats.key_constructions();
            next.max_key_component_scans = stats.key_component_scans();
            next.max_key_integer_bit_work = stats.key_integer_bit_work();
            next.max_key_prospective_retained_integer_bits =
                stats.key_prospective_retained_integer_bits();
            next.max_key_prospective_retained_bytes = stats.key_prospective_retained_bytes();
            next.max_key_observed_retained_integer_bits =
                stats.key_observed_retained_integer_bits();
            next.max_key_observed_retained_bytes = stats.key_observed_retained_bytes();
            next.max_sort_passes = stats.sort_passes();
            next.max_sort_comparisons = stats
                .group_cases()
                .checked_mul(stats.sort_passes())
                .unwrap();
            next.max_sort_comparison_integer_bit_work = stats.sort_comparison_integer_bit_work();
            next.max_sort_moves = stats.sort_moves();
            next.max_permutation_validation_scans = stats.permutation_validation_scans();
            next.max_manifest_bytes = stats.manifest_bytes();
            next.max_owner_retained_bytes = stats.owner_retained_bytes();
            next.max_peak_scratch_bytes = stats.peak_scratch_bytes();
            if next == limits {
                return (limits, plan);
            }
            limits = next;
        }
        panic!("exact solve-plan limits did not converge")
    }

    fn assert_resource_rejected(
        result: Result<
            GeneratedAffineResidualGroupSolvePlan,
            GeneratedAffineResidualGroupSolvePlanError,
        >,
    ) {
        assert!(matches!(
            result,
            Err(GeneratedAffineResidualGroupSolvePlanError::ResourceLimit { .. })
        ));
    }

    #[test]
    fn direct_frame_replay_keeps_authority_and_source_allocation_boundaries_distinct() {
        let fixture_name = "solve-plan-direct-foreign-source-allocation";
        let (family, context, terminal, authority, frame) = direct_fixture(fixture_name);
        assert_eq!(
            frame.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA
        );
        frame
            .replay_for_source_authority(&family, &context, &authority)
            .unwrap();

        let same_source_other_authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new_direct_formula_singleton(
                &family,
                &context,
                Arc::clone(&terminal),
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        assert!(authority.same_source_allocation_as(&same_source_other_authority));
        frame
            .replay_for_source_authority(&family, &context, &same_source_other_authority)
            .unwrap();
        assert!(matches!(
            frame.replay(&family, &context, &same_source_other_authority),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation)
        ));

        let (independent_family, independent_context, _, independent_authority, _) =
            direct_fixture(fixture_name);
        assert_eq!(
            family.fingerprint_ref(),
            independent_family.fingerprint_ref()
        );
        assert_eq!(context.fingerprint(), independent_context.fingerprint());
        assert_eq!(authority.sector(), independent_authority.sector());
        assert_eq!(authority.ordering(), independent_authority.ordering());
        assert_eq!(authority.arity(), independent_authority.arity());
        assert_eq!(
            authority.source_row_count(),
            independent_authority.source_row_count()
        );
        assert!(!authority.same_source_allocation_as(&independent_authority));
        assert!(matches!(
            frame.replay_for_source_authority(&family, &context, &independent_authority),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation)
        ));
    }

    #[test]
    fn direct_singleton_plan_has_no_inventory_and_is_schema_isolated_from_legacy() {
        let (family, context, _, authority, frame) =
            direct_fixture("solve-plan-direct-singleton-schema");
        let direct = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new_direct_formula_singleton(
                &family,
                &context,
                Arc::clone(&authority),
                Arc::clone(&frame),
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            direct.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V2_SCHEMA
        );
        assert_eq!(
            direct.physical_frame().schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA
        );
        assert!(direct.inventory().is_none());
        assert_eq!(direct.stats().inventory_allocation_comparisons(), 0);
        assert_eq!(
            direct.stats().retained_parent_references(),
            DIRECT_RETAINED_PARENT_REFERENCES
        );
        assert_eq!(direct.targets().len(), 1);
        assert_eq!(direct.targets()[0].solve_ordinal(), 0);
        assert_eq!(direct.targets()[0].inventory_position(), 0);
        assert_eq!(direct.targets()[0].case_ordinal(), 0);
        assert!(
            direct
                .stable_manifest()
                .starts_with(GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V2_SCHEMA)
        );
        assert!(
            direct
                .stable_manifest()
                .contains("source=direct-formula-singleton")
        );
        direct
            .replay_direct_formula_singleton(
                &family,
                &context,
                &authority,
                &frame,
                GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            )
            .unwrap();

        let direct_catalog = GeneratedAffineResidualGroupExactTargetCatalog::try_new(
            &family,
            &context,
            Arc::clone(&direct),
            GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
        )
        .unwrap();
        assert_eq!(
            direct_catalog.source_kind(),
            GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
        );
        assert_eq!(
            direct_catalog.schema(),
            super::super::targets::GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V2_SCHEMA
        );
        assert_eq!(direct_catalog.len(), 1);
        assert!(direct_catalog.same_plan_allocation(&direct));
        direct_catalog.replay(&family, &context, &direct).unwrap();

        let (legacy_family, legacy_context, inventory, legacy_authority, legacy_frame) =
            singleton_fixture("solve-plan-legacy-singleton-schema");
        let legacy = GeneratedAffineResidualGroupSolvePlan::try_new(
            &legacy_family,
            &legacy_context,
            Arc::clone(&inventory),
            Arc::clone(&legacy_authority),
            Arc::clone(&legacy_frame),
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
        )
        .unwrap();
        assert_eq!(
            legacy.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA
        );
        assert_eq!(
            legacy.physical_frame().schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA
        );
        assert!(legacy.inventory().is_some());
        assert!(
            legacy
                .stable_manifest()
                .starts_with(GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA)
        );
        assert!(
            !legacy
                .stable_manifest()
                .contains("source=direct-formula-singleton")
        );
        assert!(matches!(
            direct.replay(
                &legacy_family,
                &legacy_context,
                &inventory,
                &legacy_authority,
                &legacy_frame,
                GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch)
        ));
        assert!(matches!(
            legacy.replay_direct_formula_singleton(
                &family,
                &context,
                &authority,
                &frame,
                GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch)
        ));
    }

    #[test]
    fn authenticated_natural_group_matches_independent_physical_key_oracle() {
        let private_name = "solve-plan-natural-two-loop-private";
        let (family, context, inventory, authority, frame) = fixture(private_name);
        let plan = GeneratedAffineResidualGroupSolvePlan::try_new(
            &family,
            &context,
            Arc::clone(&inventory),
            Arc::clone(&authority),
            Arc::clone(&frame),
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
        )
        .unwrap();
        let group = authority.authenticated_group_view(&context).unwrap();
        let mut oracle = group
            .case_ordinals()
            .iter()
            .enumerate()
            .map(|(inventory_position, &case_ordinal)| {
                let key = frame
                    .key_for_physical(
                        frame
                            .anchor_offset(inventory_position, case_ordinal)
                            .unwrap(),
                    )
                    .unwrap();
                (key, inventory_position, case_ordinal)
            })
            .collect::<Vec<_>>();
        oracle.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        assert_eq!(
            plan.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA
        );
        assert!(plan.same_parent_allocations(&inventory, &authority, &frame));
        assert_eq!(
            plan.inventory().unwrap().as_ref().schema(),
            inventory.schema()
        );
        assert_eq!(plan.authority().case_ordinal(), authority.case_ordinal());
        assert_eq!(plan.physical_frame().group_ordinal(), frame.group_ordinal());
        assert_eq!(plan.group_ordinal(), group.ordinal());
        assert_eq!(plan.anchor_case_ordinal(), group.anchor_case_ordinal());
        assert_eq!(plan.free_positions(), group.free_positions());
        assert_eq!(plan.targets().len(), oracle.len());
        for (solve_ordinal, (target, expected)) in plan.targets().iter().zip(&oracle).enumerate() {
            assert_eq!(target.solve_ordinal(), solve_ordinal);
            assert_eq!(target.inventory_position(), expected.1);
            assert_eq!(target.case_ordinal(), expected.2);
            frame.replay_key(&expected.0).unwrap();
        }
        assert!(
            plan.targets()
                .windows(2)
                .all(|pair| pair[0].solve_ordinal() < pair[1].solve_ordinal())
        );
        assert_eq!(plan.stable_manifest().len(), plan.stats().manifest_bytes());
        assert!(plan.stable_manifest().contains(TARGET_ORDER_V1_ID));
        plan.replay(
            &family,
            &context,
            &inventory,
            &authority,
            &frame,
            GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
        )
        .unwrap();

        let debug = format!("{plan:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(private_name));
        assert!(!debug.contains("m2"));
        let locator_debug = format!("{:?}", plan.targets()[0]);
        assert!(locator_debug.contains("private_inventory_position"));
        assert!(locator_debug.contains("private_case_ordinal"));
        assert!(locator_debug.contains("<redacted>"));
    }

    #[test]
    fn natural_singleton_has_exact_move_admission_and_stable_manifest_width() {
        let (family, context, inventory, authority, frame) =
            singleton_fixture("solve-plan-natural-singleton-private");
        let default_plan = GeneratedAffineResidualGroupSolvePlan::try_new(
            &family,
            &context,
            Arc::clone(&inventory),
            Arc::clone(&authority),
            Arc::clone(&frame),
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
        )
        .unwrap();
        let (exact, exact_plan) = exact_plan(&family, &context, &inventory, &authority, &frame);
        let stats = exact_plan.stats();
        assert_eq!(stats.group_cases(), 1);
        assert_eq!(stats.target_locators(), 1);
        assert_eq!(stats.sort_passes(), 0);
        assert_eq!(stats.sort_comparisons(), 0);
        assert_eq!(stats.sort_moves(), 1);
        assert_eq!(exact.max_sort_moves, 1);
        assert_eq!(
            default_plan.stable_manifest().len(),
            exact_plan.stable_manifest().len(),
            "fixed-width limit fields must not feed decimal digit widths back into exact limits"
        );

        let mut below = exact;
        below.max_sort_moves = 0;
        assert_resource_rejected(GeneratedAffineResidualGroupSolvePlan::try_new(
            &family,
            &context,
            Arc::clone(&inventory),
            Arc::clone(&authority),
            Arc::clone(&frame),
            below,
        ));
        exact_plan
            .replay(
                &family,
                &context,
                &inventory,
                &authority,
                &frame,
                GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            )
            .unwrap();
    }

    #[test]
    fn arbitrary_precision_physical_keys_use_sort_entry_and_permutation_path() {
        let (_family, context, _inventory, authority, frame) =
            fixture("solve-plan-wide-sort-entry-private");
        let group = authority.authenticated_group_view(&context).unwrap();
        assert!(group.case_ordinals().len() >= 2);

        let mut positive = MultiPrecisionInteger::from(1);
        positive <<= 300_u32;
        let mut negative = MultiPrecisionInteger::from(1);
        negative <<= 332_u32;
        let positive = Integer::Large(positive);
        let negative = Integer::Large(-negative);
        let mut entries = Vec::with_capacity(group.case_ordinals().len());
        for (inventory_position, &case_ordinal) in group.case_ordinals().iter().enumerate() {
            let mut values = vec![Integer::from(0); frame.arity()];
            if inventory_position == 0 {
                values[0] = positive.clone();
            } else if inventory_position == 1 {
                values[0] = negative.clone();
            }
            let key = frame
                .test_key_for_borrowed_physical_values(&values)
                .unwrap();
            entries.push(SortEntry {
                inventory_position,
                case_ordinal,
                key,
            });
        }
        assert_eq!(
            integer_magnitude_bits(&entries[0].key.shift().values()[0]).unwrap(),
            301
        );
        assert!(!entries[0].key.shift().values()[0].is_negative());
        assert_eq!(
            integer_magnitude_bits(&entries[1].key.shift().values()[0]).unwrap(),
            333
        );
        assert!(entries[1].key.shift().values()[0].is_negative());

        let mut oracle = (0..entries.len()).collect::<Vec<_>>();
        oracle.sort_by(|&left, &right| compare_sort_entries(&entries[left], &entries[right]));
        let limits = GeneratedAffineResidualGroupSolvePlanLimits::default();
        let mut stats = GeneratedAffineResidualGroupSolvePlanStats::default();
        let ordered = stable_merge_sort_positions(&entries, limits, &mut stats).unwrap();
        assert_eq!(ordered, oracle);
        assert!(stats.sort_comparisons() > 0);

        let targets = ordered
            .iter()
            .enumerate()
            .map(|(solve_ordinal, &entry_position)| {
                let entry = &entries[entry_position];
                GeneratedAffineResidualGroupSolveTargetLocator {
                    solve_ordinal,
                    inventory_position: entry.inventory_position,
                    case_ordinal: entry.case_ordinal,
                }
            })
            .collect::<Vec<_>>();
        validate_target_permutation(group, &targets, limits, &mut stats).unwrap();
        assert_eq!(stats.permutation_validation_scans(), targets.len() * 2);
    }

    #[test]
    fn stable_merge_sort_is_ascending_stable_and_not_raw_reverse() {
        let values = [1_i32, 3, 2];
        let mut stats = GeneratedAffineResidualGroupSolvePlanStats::default();
        let order = stable_merge_sort_positions_by(
            values.len(),
            |left, right| {
                Ok(values[left]
                    .cmp(&values[right])
                    .then_with(|| left.cmp(&right)))
            },
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert_eq!(order, [0, 2, 1]);
        assert_ne!(order, [2, 1, 0]);

        let equal_values = [1_i32, 1, 0, 1];
        let mut equal_stats = GeneratedAffineResidualGroupSolvePlanStats::default();
        let stable = stable_merge_sort_positions_by(
            equal_values.len(),
            |left, right| Ok(equal_values[left].cmp(&equal_values[right])),
            GeneratedAffineResidualGroupSolvePlanLimits::default(),
            &mut equal_stats,
        )
        .unwrap();
        assert_eq!(stable, [2, 0, 1, 3]);
        assert!(stats.sort_comparisons() > 0);
        assert!(equal_stats.sort_moves() > 0);
    }

    #[test]
    fn exact_limits_replay_identity_and_every_positive_one_below_are_enforced() {
        let private_name = "solve-plan-exact-limits-private";
        let (family, context, inventory, authority, frame) = fixture(private_name);
        let (exact, plan) = exact_plan(&family, &context, &inventory, &authority, &frame);
        let stats = plan.stats();
        assert_eq!(plan.limits(), exact);
        assert!(stats.key_observed_retained_bytes() <= stats.key_prospective_retained_bytes());
        assert!(
            stats.key_observed_retained_integer_bits()
                <= stats.key_prospective_retained_integer_bits()
        );
        assert!(stats.sort_comparison_integer_bit_work() > 0);

        macro_rules! one_below {
            ($field:ident, $demand:expr) => {{
                let demand = $demand;
                if demand > 0 {
                    let mut below = exact;
                    below.$field = demand - 1;
                    assert_resource_rejected(GeneratedAffineResidualGroupSolvePlan::try_new(
                        &family,
                        &context,
                        Arc::clone(&inventory),
                        Arc::clone(&authority),
                        Arc::clone(&frame),
                        below,
                    ));
                }
            }};
        }
        one_below!(max_scope_comparison_bytes, stats.scope_comparison_bytes());
        one_below!(
            max_inventory_allocation_comparisons,
            stats.inventory_allocation_comparisons()
        );
        one_below!(max_frame_replays, stats.frame_replays());
        one_below!(max_group_authentications, stats.group_authentications());
        one_below!(
            max_retained_parent_references,
            stats.retained_parent_references()
        );
        one_below!(max_group_cases, stats.group_cases());
        one_below!(max_arity, stats.arity());
        one_below!(max_free_positions, stats.free_positions());
        one_below!(max_target_locators, stats.target_locators());
        one_below!(
            max_key_aggregate_preflights,
            stats.key_aggregate_preflights()
        );
        one_below!(max_key_constructions, stats.key_constructions());
        one_below!(max_key_component_scans, stats.key_component_scans());
        one_below!(max_key_integer_bit_work, stats.key_integer_bit_work());
        one_below!(
            max_key_prospective_retained_integer_bits,
            stats.key_prospective_retained_integer_bits()
        );
        one_below!(
            max_key_prospective_retained_bytes,
            stats.key_prospective_retained_bytes()
        );
        one_below!(
            max_key_observed_retained_integer_bits,
            stats.key_observed_retained_integer_bits()
        );
        one_below!(
            max_key_observed_retained_bytes,
            stats.key_observed_retained_bytes()
        );
        one_below!(max_sort_passes, stats.sort_passes());
        one_below!(
            max_sort_comparisons,
            stats.group_cases() * stats.sort_passes()
        );
        one_below!(
            max_sort_comparison_integer_bit_work,
            stats.sort_comparison_integer_bit_work()
        );
        one_below!(max_sort_moves, stats.sort_moves());
        one_below!(
            max_permutation_validation_scans,
            stats.permutation_validation_scans()
        );
        one_below!(max_manifest_bytes, stats.manifest_bytes());
        one_below!(max_owner_retained_bytes, stats.owner_retained_bytes());
        one_below!(max_peak_scratch_bytes, stats.peak_scratch_bytes());

        let exact_replay = GeneratedAffineResidualGroupSolvePlanReplayLimits {
            max_parent_allocation_comparisons: LEGACY_RETAINED_PARENT_REFERENCES,
            max_combined_owner_bytes: stats.replay_combined_owner_bytes(),
            max_payload_comparison_units: stats.payload_comparison_units(),
            max_payload_comparison_bytes: stats.payload_comparison_bytes(),
        };
        plan.replay(
            &family,
            &context,
            &inventory,
            &authority,
            &frame,
            exact_replay,
        )
        .unwrap();
        for replay_limits in [
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_parent_allocation_comparisons: LEGACY_RETAINED_PARENT_REFERENCES - 1,
                ..exact_replay
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_combined_owner_bytes: stats.replay_combined_owner_bytes() - 1,
                ..exact_replay
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_payload_comparison_units: stats.payload_comparison_units() - 1,
                ..exact_replay
            },
            GeneratedAffineResidualGroupSolvePlanReplayLimits {
                max_payload_comparison_bytes: stats.payload_comparison_bytes() - 1,
                ..exact_replay
            },
        ] {
            assert!(matches!(
                plan.replay(
                    &family,
                    &context,
                    &inventory,
                    &authority,
                    &frame,
                    replay_limits,
                ),
                Err(GeneratedAffineResidualGroupSolvePlanError::ResourceLimit { .. })
            ));
        }

        let foreign_authority = Arc::new(authority.as_ref().clone());
        assert!(!Arc::ptr_eq(&authority, &foreign_authority));
        assert!(matches!(
            plan.replay(
                &family,
                &context,
                &inventory,
                &foreign_authority,
                &frame,
                exact_replay,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::WrongAuthorityAllocation)
        ));
        assert!(matches!(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                Arc::clone(&foreign_authority),
                Arc::clone(&frame),
                exact,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::WrongAuthorityAllocation)
        ));
        let foreign_frame = Arc::new(frame.as_ref().clone());
        assert!(!Arc::ptr_eq(&frame, &foreign_frame));
        assert!(matches!(
            plan.replay(
                &family,
                &context,
                &inventory,
                &authority,
                &foreign_frame,
                exact_replay,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::WrongFrameAllocation)
        ));
        let (_, _, foreign_inventory, _, _) = fixture(private_name);
        assert!(!Arc::ptr_eq(&inventory, &foreign_inventory));
        assert!(matches!(
            plan.replay(
                &family,
                &context,
                &foreign_inventory,
                &authority,
                &frame,
                exact_replay,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation)
        ));

        let mut wrong_schema = plan.clone();
        wrong_schema.schema = "wrong-solve-plan-schema";
        assert!(matches!(
            wrong_schema.replay(
                &family,
                &context,
                &inventory,
                &authority,
                &frame,
                exact_replay,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::SchemaMismatch)
        ));
        let mut wrong_order = plan.clone();
        Arc::make_mut(&mut wrong_order.targets)[0].solve_ordinal = usize::MAX;
        assert!(matches!(
            wrong_order.replay(
                &family,
                &context,
                &inventory,
                &authority,
                &frame,
                exact_replay,
            ),
            Err(GeneratedAffineResidualGroupSolvePlanError::ReplayMismatch)
        ));

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GeneratedAffineResidualGroupSolvePlan>();
        let weak_inventory: Weak<GeneratedAffineResidualCaseInventoryCertificate> =
            Arc::downgrade(&inventory);
        let weak_authority: Weak<GeneratedAffineResidualCaseAuthority> = Arc::downgrade(&authority);
        let weak_frame: Weak<GeneratedAffineResidualGroupPhysicalFrame> = Arc::downgrade(&frame);
        drop(foreign_frame);
        drop(foreign_authority);
        drop(wrong_schema);
        drop(wrong_order);
        drop(inventory);
        drop(authority);
        drop(frame);
        assert!(weak_inventory.upgrade().is_some());
        assert!(weak_authority.upgrade().is_some());
        assert!(weak_frame.upgrade().is_some());
        drop(plan);
        assert!(weak_inventory.upgrade().is_none());
        assert!(weak_authority.upgrade().is_none());
        assert!(weak_frame.upgrade().is_none());
    }
}
