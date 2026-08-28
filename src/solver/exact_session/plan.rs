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
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-solve-plan-v3";
const TARGET_ORDER_V1_ID: &str = "stable-ascending-physical-key-then-inventory-position-v1";

const INVENTORY_ALLOCATION_COMPARISONS: usize = 1;
const FRAME_REPLAYS: usize = 1;
const GROUP_AUTHENTICATIONS: usize = 1;
const INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES: usize = 3;
const SINGLETON_RETAINED_PARENT_REFERENCES: usize = 2;
const LIMIT_SCALAR_FIELDS: usize = 25;
const STATS_SCALAR_FIELDS: usize = 29;

const fn solve_plan_schema_for_source(
    source: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V1_SCHEMA
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
            max_retained_parent_references: INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES,
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
            max_parent_allocation_comparisons: INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES,
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
    InitialInventory(Arc<GeneratedAffineResidualCaseInventoryCertificate>),
    CommittedExceptionalSingleton,
}

impl GeneratedAffineResidualGroupSolvePlanSource {
    const fn kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        match self {
            Self::InitialInventory(_) => {
                GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory
            }
            Self::CommittedExceptionalSingleton => {
                GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
            }
        }
    }

    const fn retained_parent_references(&self) -> usize {
        match self {
            Self::InitialInventory(_) => INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES,
            Self::CommittedExceptionalSingleton => SINGLETON_RETAINED_PARENT_REFERENCES,
        }
    }

    const fn inventory_allocation_comparisons(&self) -> usize {
        match self {
            Self::InitialInventory(_) => INVENTORY_ALLOCATION_COMPARISONS,
            Self::CommittedExceptionalSingleton => 0,
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
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(inventory),
            authority,
            physical_frame,
            limits,
        )
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
    /// inventory arm presents the retained inventory while the exceptional
    /// arm presents the retained singleton authority and frame. Callers cannot
    /// use it to substitute an independently compiled, payload-equal source.
    pub(crate) fn replay_retained_source(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        replay_limits: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    ) -> Result<(), GeneratedAffineResidualGroupSolvePlanError> {
        match &self.source {
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(inventory) => self
                .replay(
                    family,
                    context,
                    inventory,
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
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(inventory) => {
                Some(inventory)
            }
            GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => None,
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
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(retained)
                if Arc::ptr_eq(retained, inventory)
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
                INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES,
                replay_limits.max_parent_allocation_comparisons,
            )?;
            if !matches!(
                &self.source,
                GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(retained)
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
                INITIAL_INVENTORY_RETAINED_PARENT_REFERENCES,
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
                SINGLETON_RETAINED_PARENT_REFERENCES,
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
                SINGLETON_RETAINED_PARENT_REFERENCES,
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
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(left),
            GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(right),
        ) => Arc::ptr_eq(left, right),
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
        GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(inventory) => checked_sum(
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
        GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => checked_sum(
            "solve-plan scope comparison bytes",
            [
                family.fingerprint_ref().len(),
                authority.family_fingerprint().len(),
                context.fingerprint().len(),
                authority.context_fingerprint().len(),
            ],
        )?,
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
        GeneratedAffineResidualGroupSolvePlanSource::InitialInventory(inventory) => {
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
        GeneratedAffineResidualGroupSolvePlanSource::CommittedExceptionalSingleton => {
            if authority.source_kind()
                != GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
            {
                return Err(GeneratedAffineResidualGroupSolvePlanError::WrongInventoryAllocation);
            }
        }
    }
    let expected_frame_schema = match source.kind() {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA
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
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {}
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
