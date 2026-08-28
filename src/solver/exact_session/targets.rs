//! Plan-bound exact target catalog and inert target state.
//!
//! This module is the sealed bridge from one persisted generated-affine solve
//! order to the exact targets that later recentering will consume.  It is
//! deliberately topology-neutral: every target is discovered from the exact
//! solve-plan allocation, authenticated through that plan's retained source
//! authority, and projected through the existing case-premises compiler.
//!
//! Neither source-schema version publishes a rule, performs recentering, or
//! exposes a caller-owned bitmap. Equality-bearing cases remain a typed
//! non-ready outcome. The state
//! owner starts with every target unresolved and advances only by immutable,
//! allocation-bound successors that either preserve every disposition or
//! consume one authenticated Ready target.

use std::fmt;
use std::mem::{align_of, size_of};
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::database::GeneratedAffineResidualGroupExactTargetStateBinding;
use super::plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanReplayLimits,
    GeneratedAffineResidualGroupSolveTargetLocator,
};
use super::session::GeneratedAffineResidualGroupExactSessionDatabaseCapability;
use crate::generated_affine_residual_case_premises::{
    GeneratedAffineResidualCaseEqualityRefinementCertificate,
    GeneratedAffineResidualCasePremisesCertificate, GeneratedAffineResidualCasePremisesLimits,
    GeneratedAffineResidualCasePremisesOutcome, compile_generated_affine_residual_case_premises,
};
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
    GeneratedAffineResidualCaseAuthoritySourceKind,
    GeneratedAffineResidualSameGroupTargetCaseLimits,
    GeneratedAffineResidualSameGroupTargetCasesLimits,
    GeneratedAffineResidualSameGroupTargetHandleLimits,
};
use crate::{IntegralFamily, ParametricCoefficientContext, ParametricNonZeroCondition};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-catalog-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-catalog-v2";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-catalog-v3";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-state-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-state-v2";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V3_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-target-state-v3";

const TARGET_LOCATOR_COMPARISONS: usize = 9;
const DIRECT_TARGET_LOCATOR_COMPARISONS: usize = 9;

const fn exact_target_catalog_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V2_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V3_SCHEMA
        }
    }
}

const fn exact_target_state_schema_for_source(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V1_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V2_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V3_SCHEMA
        }
    }
}

const fn is_singleton_source_kind(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> bool {
    matches!(
        source_kind,
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            | GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
    )
}

/// Process-unique identity for exact target-state allocations.  The nonce is
/// never exposed or accepted from a caller; the live database/session binding
/// retains and compares the exact state `Arc` instead.
static NEXT_EXACT_TARGET_STATE_NONCE: AtomicU64 = AtomicU64::new(1);

/// Complete construction/replay envelope for one plan-bound exact catalog.
///
/// Child limits remain explicit because target-authority replay and premise
/// compilation own the potentially large inventory/Symbolica work.  The outer
/// counters bound the exact repeated-call cardinalities and the retained-byte
/// fields bound this owner's Rust-visible payload.  Shared plan/source/frame
/// graphs are charged by their existing owners and are not duplicated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactTargetCatalogLimits {
    pub(crate) solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits,
    pub(crate) target_cases: GeneratedAffineResidualSameGroupTargetCasesLimits,
    pub(crate) target_handle: GeneratedAffineResidualSameGroupTargetHandleLimits,
    pub(crate) target_case: GeneratedAffineResidualSameGroupTargetCaseLimits,
    pub(crate) target_authority: GeneratedAffineResidualCaseAuthorityLimits,
    pub(crate) premises: GeneratedAffineResidualCasePremisesLimits,
    pub(crate) max_plan_replays: usize,
    pub(crate) max_same_group_target_collections: usize,
    pub(crate) max_targets: usize,
    pub(crate) max_locator_comparisons: usize,
    pub(crate) max_target_handle_resolutions: usize,
    pub(crate) max_target_case_authentications: usize,
    pub(crate) max_target_authority_constructions: usize,
    pub(crate) max_premises_compilations: usize,
    pub(crate) max_ready_targets: usize,
    pub(crate) max_equality_refinement_targets: usize,
    pub(crate) max_retained_plan_references: usize,
    pub(crate) max_retained_authority_references: usize,
    pub(crate) max_owner_retained_byte_envelope: usize,
    pub(crate) max_peak_staging_byte_envelope: usize,
}

impl Default for GeneratedAffineResidualGroupExactTargetCatalogLimits {
    fn default() -> Self {
        const LARGE: usize = 256_000_000;
        Self {
            solve_plan_replay: GeneratedAffineResidualGroupSolvePlanReplayLimits::default(),
            target_cases: GeneratedAffineResidualSameGroupTargetCasesLimits::default(),
            target_handle: GeneratedAffineResidualSameGroupTargetHandleLimits::default(),
            target_case: GeneratedAffineResidualSameGroupTargetCaseLimits::default(),
            target_authority: GeneratedAffineResidualCaseAuthorityLimits::default(),
            premises: GeneratedAffineResidualCasePremisesLimits::default(),
            max_plan_replays: 1,
            max_same_group_target_collections: 1,
            max_targets: LARGE,
            max_locator_comparisons: LARGE.saturating_mul(TARGET_LOCATOR_COMPARISONS),
            max_target_handle_resolutions: LARGE,
            max_target_case_authentications: LARGE,
            max_target_authority_constructions: LARGE,
            max_premises_compilations: LARGE,
            max_ready_targets: LARGE,
            max_equality_refinement_targets: LARGE,
            max_retained_plan_references: LARGE.saturating_add(1),
            max_retained_authority_references: LARGE.saturating_mul(2),
            max_owner_retained_byte_envelope: usize::MAX / 4,
            max_peak_staging_byte_envelope: usize::MAX / 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactTargetCatalogStats {
    plan_replays: usize,
    same_group_target_collections: usize,
    targets: usize,
    locator_comparisons: usize,
    target_handle_resolutions: usize,
    target_case_authentications: usize,
    target_authority_constructions: usize,
    premises_compilations: usize,
    ready_targets: usize,
    equality_refinement_targets: usize,
    retained_plan_references: usize,
    retained_authority_references: usize,
    owner_retained_byte_envelope: usize,
    peak_staging_byte_envelope: usize,
}

impl GeneratedAffineResidualGroupExactTargetCatalogStats {
    pub(crate) const fn targets(self) -> usize {
        self.targets
    }
    pub(crate) const fn ready_targets(self) -> usize {
        self.ready_targets
    }
    pub(crate) const fn equality_refinement_targets(self) -> usize {
        self.equality_refinement_targets
    }
    pub(crate) const fn owner_retained_byte_envelope(self) -> usize {
        self.owner_retained_byte_envelope
    }
    pub(crate) const fn peak_staging_byte_envelope(self) -> usize {
        self.peak_staging_byte_envelope
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactTargetError {
    WrongDatabaseAllocation,
    WrongPredecessorTransition,
    WrongPlanAllocation,
    WrongFrameAllocation,
    WrongSourceStateAllocation,
    WrongGroup,
    WrongDatabaseEpoch,
    WrongStateVersion,
    StateVersionOverflow,
    StateIdentityExhaustion,
    PlanReplay,
    TargetCollection,
    TargetResolution,
    TargetAuthority,
    Premises,
    MalformedTargetOrder,
    TargetOutOfRange,
    TargetConsumed,
    ReplayMismatch,
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

impl GeneratedAffineResidualGroupExactTargetError {
    const fn kind(self) -> &'static str {
        match self {
            Self::WrongDatabaseAllocation => "WrongDatabaseAllocation",
            Self::WrongPredecessorTransition => "WrongPredecessorTransition",
            Self::WrongPlanAllocation => "WrongPlanAllocation",
            Self::WrongFrameAllocation => "WrongFrameAllocation",
            Self::WrongSourceStateAllocation => "WrongSourceStateAllocation",
            Self::WrongGroup => "WrongGroup",
            Self::WrongDatabaseEpoch => "WrongDatabaseEpoch",
            Self::WrongStateVersion => "WrongStateVersion",
            Self::StateVersionOverflow => "StateVersionOverflow",
            Self::StateIdentityExhaustion => "StateIdentityExhaustion",
            Self::PlanReplay => "PlanReplay",
            Self::TargetCollection => "TargetCollection",
            Self::TargetResolution => "TargetResolution",
            Self::TargetAuthority => "TargetAuthority",
            Self::Premises => "Premises",
            Self::MalformedTargetOrder => "MalformedTargetOrder",
            Self::TargetOutOfRange => "TargetOutOfRange",
            Self::TargetConsumed => "TargetConsumed",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactTargetError")
            .field("kind", &self.kind())
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongDatabaseAllocation => {
                "exact target state belongs to another database allocation"
            }
            Self::WrongPredecessorTransition => {
                "exact target successor was not minted from this predecessor transition"
            }
            Self::WrongPlanAllocation => {
                "exact target owner belongs to another solve-plan allocation"
            }
            Self::WrongFrameAllocation => {
                "exact target owner belongs to another physical-frame allocation"
            }
            Self::WrongSourceStateAllocation => {
                "retained exact target belongs to another source-state allocation"
            }
            Self::WrongGroup => "exact target owner group binding mismatch",
            Self::WrongDatabaseEpoch => "exact target state database epoch mismatch",
            Self::WrongStateVersion => "exact target state version mismatch",
            Self::StateVersionOverflow => "exact target state version overflow",
            Self::StateIdentityExhaustion => "exact target state allocation identity exhausted",
            Self::PlanReplay => "exact target solve-plan replay failed",
            Self::TargetCollection => "exact target same-group collection authentication failed",
            Self::TargetResolution => "exact target locator resolution failed",
            Self::TargetAuthority => "exact target case authority authentication failed",
            Self::Premises => "exact target premise/domain compilation failed",
            Self::MalformedTargetOrder => "exact target persisted order is malformed",
            Self::TargetOutOfRange => "exact target solve ordinal is out of range",
            Self::TargetConsumed => "exact target has already been consumed",
            Self::ReplayMismatch => "exact target retained payload replay mismatch",
            Self::ResourceLimit { .. } => "exact target resource limit exceeded",
            Self::ResourceCountOverflow { .. } => "exact target resource count overflow",
            Self::AllocationFailure { .. } => "exact target allocation failed",
            Self::SymbolicaPanic => "Symbolica panicked inside the exact target boundary",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactTargetError {}

pub(crate) struct GeneratedAffineResidualGroupReadyExactTarget {
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    locator: GeneratedAffineResidualGroupSolveTargetLocator,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    domain: GeneratedAffineResidualCasePremisesCertificate,
}

pub(crate) struct GeneratedAffineResidualGroupEqualityRefinementExactTarget {
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    locator: GeneratedAffineResidualGroupSolveTargetLocator,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    refinement: GeneratedAffineResidualCaseEqualityRefinementCertificate,
}

enum GeneratedAffineResidualGroupExactTargetOutcome {
    Ready(GeneratedAffineResidualGroupReadyExactTarget),
    RequiresAffineEqualityRefinement(GeneratedAffineResidualGroupEqualityRefinementExactTarget),
}

impl GeneratedAffineResidualGroupExactTargetOutcome {
    const fn locator(&self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        match self {
            Self::Ready(target) => target.locator,
            Self::RequiresAffineEqualityRefinement(target) => target.locator,
        }
    }

    fn plan(&self) -> &Arc<GeneratedAffineResidualGroupSolvePlan> {
        match self {
            Self::Ready(target) => &target.plan,
            Self::RequiresAffineEqualityRefinement(target) => &target.plan,
        }
    }

    fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        match self {
            Self::Ready(target) => &target.authority,
            Self::RequiresAffineEqualityRefinement(target) => &target.authority,
        }
    }

    fn child_retained_bytes(&self) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
        match self {
            Self::Ready(target) => target
                .domain
                .stats()
                .retained_bytes()
                .checked_sub(size_of::<GeneratedAffineResidualCasePremisesCertificate>()),
            Self::RequiresAffineEqualityRefinement(target) => target
                .refinement
                .stats()
                .retained_bytes()
                .checked_sub(size_of::<
                    GeneratedAffineResidualCaseEqualityRefinementCertificate,
                >()),
        }
        .ok_or(
            GeneratedAffineResidualGroupExactTargetError::ResourceCountOverflow {
                resource: "exact target child retained bytes",
            },
        )
    }

    const fn child_peak_bytes(&self) -> usize {
        match self {
            Self::Ready(target) => target.domain.stats().peak_scratch_byte_envelope(),
            Self::RequiresAffineEqualityRefinement(target) => {
                target.refinement.stats().peak_scratch_byte_envelope()
            }
        }
    }
}

/// Immutable plan-allocation-bound target catalog in persisted solve order.
pub(crate) struct GeneratedAffineResidualGroupExactTargetCatalog {
    schema: &'static str,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
    group_ordinal: usize,
    targets: Vec<GeneratedAffineResidualGroupExactTargetOutcome>,
    limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    stats: GeneratedAffineResidualGroupExactTargetCatalogStats,
}

impl fmt::Debug for GeneratedAffineResidualGroupExactTargetCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactTargetCatalog")
            .field("schema", &self.schema)
            .field("group_ordinal", &self.group_ordinal)
            .field("target_count", &self.targets.len())
            .field("ready_targets", &self.stats.ready_targets)
            .field(
                "equality_refinement_targets",
                &self.stats.equality_refinement_targets,
            )
            .field("private_plan", &"<redacted>")
            .field("private_targets", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualGroupSolvePlan {
    pub(crate) fn compile_exact_target_catalog(
        self: &Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetCatalog,
        GeneratedAffineResidualGroupExactTargetError,
    > {
        GeneratedAffineResidualGroupExactTargetCatalog::try_new(
            family,
            context,
            Arc::clone(self),
            limits,
        )
    }
}

impl GeneratedAffineResidualGroupExactTargetCatalog {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactTargetError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_unwind_boundary(family, context, plan, limits)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    fn try_new_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactTargetError> {
        let source_kind = plan.source_kind();
        preflight_catalog_counts(source_kind, plan.targets().len(), limits)?;
        plan.replay_retained_source(family, context, limits.solve_plan_replay)
            .map_err(|_| GeneratedAffineResidualGroupExactTargetError::PlanReplay)?;
        if is_singleton_source_kind(source_kind) {
            return Self::try_new_direct_formula_singleton_after_plan_replay(
                family, context, plan, limits,
            );
        }
        let inventory = plan
            .inventory()
            .ok_or(GeneratedAffineResidualGroupExactTargetError::PlanReplay)?;
        let target_cases = plan
            .authority()
            .same_group_target_cases(family, context, limits.target_cases)
            .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetCollection)?;
        if target_cases.group_ordinal() != plan.group_ordinal()
            || target_cases.len() != plan.targets().len()
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        }

        let mut targets = try_vec_with_capacity("exact target catalog", plan.targets().len())?;
        let base_retained = catalog_base_retained_bytes(targets.capacity())?;
        check_limit(
            "exact target owner retained byte envelope",
            base_retained,
            limits.max_owner_retained_byte_envelope,
        )?;
        let mut stats = GeneratedAffineResidualGroupExactTargetCatalogStats {
            plan_replays: 1,
            same_group_target_collections: 1,
            targets: plan.targets().len(),
            locator_comparisons: checked_mul(
                "exact target locator comparisons",
                plan.targets().len(),
                TARGET_LOCATOR_COMPARISONS,
            )?,
            target_handle_resolutions: plan.targets().len(),
            target_case_authentications: plan.targets().len(),
            target_authority_constructions: plan.targets().len(),
            premises_compilations: plan.targets().len(),
            retained_plan_references: checked_add(
                "exact target retained plan references",
                plan.targets().len(),
                1,
            )?,
            retained_authority_references: checked_mul(
                "exact target retained authority references",
                plan.targets().len(),
                2,
            )?,
            owner_retained_byte_envelope: base_retained,
            peak_staging_byte_envelope: base_retained,
            ..GeneratedAffineResidualGroupExactTargetCatalogStats::default()
        };

        for (solve_ordinal, locator) in plan.targets().iter().copied().enumerate() {
            if locator.solve_ordinal() != solve_ordinal {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }
            let handle = target_cases
                .target(locator.inventory_position(), limits.target_handle)
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetResolution)?;
            if handle.case_ordinal() != locator.case_ordinal()
                || handle.group_ordinal() != plan.group_ordinal()
                || handle.ordinal_within_group() != locator.inventory_position()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }
            let authenticated = plan
                .authority()
                .authenticated_same_group_target_case_view(
                    family,
                    context,
                    handle,
                    limits.target_case,
                )
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetResolution)?;
            let target_record = authenticated.target();
            if target_record.ordinal() != locator.case_ordinal()
                || target_record.group_ordinal() != plan.group_ordinal()
                || target_record.ordinal_within_group() != locator.inventory_position()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }

            let authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    family,
                    context,
                    Arc::clone(inventory),
                    locator.case_ordinal(),
                    limits.target_authority,
                )
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetAuthority)?,
            );
            if authority.case_ordinal() != locator.case_ordinal()
                || authority.group_ordinal() != plan.group_ordinal()
                || !authority.same_inventory_allocation_as(plan.authority())
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::TargetAuthority);
            }
            let premises = compile_generated_affine_residual_case_premises(
                family,
                context,
                Arc::clone(&authority),
                limits.premises,
            )
            .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
            let outcome = match premises {
                GeneratedAffineResidualCasePremisesOutcome::Ready(domain) => {
                    stats.ready_targets =
                        checked_add("exact ready targets", stats.ready_targets, 1)?;
                    check_limit(
                        "exact ready targets",
                        stats.ready_targets,
                        limits.max_ready_targets,
                    )?;
                    GeneratedAffineResidualGroupExactTargetOutcome::Ready(
                        GeneratedAffineResidualGroupReadyExactTarget {
                            plan: Arc::clone(&plan),
                            locator,
                            authority,
                            domain,
                        },
                    )
                }
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
                    refinement,
                ) => {
                    stats.equality_refinement_targets = checked_add(
                        "exact equality-refinement targets",
                        stats.equality_refinement_targets,
                        1,
                    )?;
                    check_limit(
                        "exact equality-refinement targets",
                        stats.equality_refinement_targets,
                        limits.max_equality_refinement_targets,
                    )?;
                    GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                        GeneratedAffineResidualGroupEqualityRefinementExactTarget {
                            plan: Arc::clone(&plan),
                            locator,
                            authority,
                            refinement,
                        },
                    )
                }
            };
            let staged_owner =
                catalog_retained_bytes(&targets, targets.capacity(), Some(&outcome))?;
            check_limit(
                "exact target owner retained byte envelope",
                staged_owner,
                limits.max_owner_retained_byte_envelope,
            )?;
            let staged_peak =
                catalog_peak_staging_bytes(&targets, targets.capacity(), Some(&outcome))?;
            check_limit(
                "exact target peak staging byte envelope",
                staged_peak,
                limits.max_peak_staging_byte_envelope,
            )?;
            stats.peak_staging_byte_envelope = stats.peak_staging_byte_envelope.max(staged_peak);
            targets.push(outcome);
        }
        stats.owner_retained_byte_envelope =
            catalog_retained_bytes(&targets, targets.capacity(), None)?;
        stats.peak_staging_byte_envelope =
            stats
                .peak_staging_byte_envelope
                .max(catalog_peak_staging_bytes(
                    &targets,
                    targets.capacity(),
                    None,
                )?);
        validate_catalog_stats(source_kind, stats, limits)?;
        Ok(Self {
            schema: exact_target_catalog_schema_for_source(source_kind),
            source_kind,
            group_ordinal: plan.group_ordinal(),
            plan,
            targets,
            limits,
            stats,
        })
    }

    fn try_new_direct_formula_singleton_after_plan_replay(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupExactTargetError> {
        let [locator] = plan.targets() else {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        };
        if locator.solve_ordinal() != 0
            || locator.inventory_position() != 0
            || locator.case_ordinal() != 0
            || plan.group_ordinal() != 0
            || plan.anchor_case_ordinal() != 0
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        }
        let source_kind = plan.source_kind();
        let authority = Arc::clone(plan.authority());
        if !is_singleton_source_kind(source_kind)
            || authority.source_kind() != source_kind
            || authority.case_ordinal() != 0
            || authority.group_ordinal() != 0
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::TargetAuthority);
        }
        let mut targets = try_vec_with_capacity("exact target catalog", 1)?;
        let base_retained = catalog_base_retained_bytes(targets.capacity())?;
        check_limit(
            "exact target owner retained byte envelope",
            base_retained,
            limits.max_owner_retained_byte_envelope,
        )?;
        check_limit(
            "exact target peak staging byte envelope",
            base_retained,
            limits.max_peak_staging_byte_envelope,
        )?;
        let mut stats = GeneratedAffineResidualGroupExactTargetCatalogStats {
            plan_replays: 1,
            same_group_target_collections: 0,
            targets: 1,
            locator_comparisons: DIRECT_TARGET_LOCATOR_COMPARISONS,
            target_handle_resolutions: 0,
            target_case_authentications: 0,
            target_authority_constructions: 0,
            premises_compilations: 1,
            retained_plan_references: 2,
            retained_authority_references: 2,
            owner_retained_byte_envelope: base_retained,
            peak_staging_byte_envelope: base_retained,
            ..GeneratedAffineResidualGroupExactTargetCatalogStats::default()
        };
        let premises = compile_generated_affine_residual_case_premises(
            family,
            context,
            Arc::clone(&authority),
            limits.premises,
        )
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
        let outcome = match premises {
            GeneratedAffineResidualCasePremisesOutcome::Ready(domain) => {
                stats.ready_targets = 1;
                GeneratedAffineResidualGroupExactTargetOutcome::Ready(
                    GeneratedAffineResidualGroupReadyExactTarget {
                        plan: Arc::clone(&plan),
                        locator: *locator,
                        authority,
                        domain,
                    },
                )
            }
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(
                refinement,
            ) => {
                stats.equality_refinement_targets = 1;
                GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                    GeneratedAffineResidualGroupEqualityRefinementExactTarget {
                        plan: Arc::clone(&plan),
                        locator: *locator,
                        authority,
                        refinement,
                    },
                )
            }
        };
        let staged_owner = catalog_retained_bytes(&targets, targets.capacity(), Some(&outcome))?;
        check_limit(
            "exact target owner retained byte envelope",
            staged_owner,
            limits.max_owner_retained_byte_envelope,
        )?;
        let staged_peak = catalog_peak_staging_bytes(&targets, targets.capacity(), Some(&outcome))?;
        check_limit(
            "exact target peak staging byte envelope",
            staged_peak,
            limits.max_peak_staging_byte_envelope,
        )?;
        targets.push(outcome);
        stats.owner_retained_byte_envelope =
            catalog_retained_bytes(&targets, targets.capacity(), None)?;
        stats.peak_staging_byte_envelope = staged_peak.max(catalog_peak_staging_bytes(
            &targets,
            targets.capacity(),
            None,
        )?);
        validate_catalog_stats(source_kind, stats, limits)?;
        Ok(Self {
            schema: exact_target_catalog_schema_for_source(source_kind),
            source_kind,
            group_ordinal: 0,
            plan,
            targets,
            limits,
            stats,
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.source_kind
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) fn len(&self) -> usize {
        self.targets.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactTargetCatalogLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactTargetCatalogStats {
        self.stats
    }
    pub(crate) fn same_plan_allocation(
        &self,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> bool {
        Arc::ptr_eq(&self.plan, plan)
    }

    /// Borrow the immutable premises of a Ready target after it has been
    /// consumed from a live target state.  The catalog is immutable; target
    /// consumption changes only the separate disposition vector.
    pub(crate) fn ready_target_premises(
        &self,
        locator: GeneratedAffineResidualGroupSolveTargetLocator,
    ) -> Option<&[ParametricNonZeroCondition]> {
        match self.targets.get(locator.solve_ordinal())? {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(target)
                if target.locator == locator =>
            {
                Some(target.domain.premises())
            }
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(_)
            | GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(_) => {
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn target_uses_exact_plan_authority_allocation_for_test(
        &self,
        solve_ordinal: usize,
    ) -> bool {
        self.targets
            .get(solve_ordinal)
            .is_some_and(|target| Arc::ptr_eq(target.authority(), self.plan.authority()))
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.replay_unwind_boundary(family, context, plan)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    fn replay_unwind_boundary(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
        if !Arc::ptr_eq(&self.plan, plan) {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation);
        }
        if self.source_kind != plan.source_kind()
            || self.schema != exact_target_catalog_schema_for_source(self.source_kind)
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        if self.group_ordinal != plan.group_ordinal() {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongGroup);
        }
        preflight_catalog_counts(self.source_kind, self.targets.len(), self.limits)?;
        plan.replay_retained_source(family, context, self.limits.solve_plan_replay)
            .map_err(|_| GeneratedAffineResidualGroupExactTargetError::PlanReplay)?;
        if is_singleton_source_kind(self.source_kind) {
            return self.replay_direct_formula_singleton_after_plan_replay(family, context, plan);
        }
        if plan.inventory().is_none() {
            return Err(GeneratedAffineResidualGroupExactTargetError::PlanReplay);
        }
        if plan.targets().len() != self.targets.len() {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        }
        let target_cases = plan
            .authority()
            .same_group_target_cases(family, context, self.limits.target_cases)
            .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetCollection)?;
        if target_cases.group_ordinal() != self.group_ordinal
            || target_cases.len() != self.targets.len()
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        }
        let mut stats = GeneratedAffineResidualGroupExactTargetCatalogStats {
            plan_replays: 1,
            same_group_target_collections: 1,
            targets: self.targets.len(),
            locator_comparisons: checked_mul(
                "exact target locator comparisons",
                self.targets.len(),
                TARGET_LOCATOR_COMPARISONS,
            )?,
            target_handle_resolutions: self.targets.len(),
            target_case_authentications: self.targets.len(),
            target_authority_constructions: self.targets.len(),
            premises_compilations: self.targets.len(),
            retained_plan_references: checked_add(
                "exact target retained plan references",
                self.targets.len(),
                1,
            )?,
            retained_authority_references: checked_mul(
                "exact target retained authority references",
                self.targets.len(),
                2,
            )?,
            ..GeneratedAffineResidualGroupExactTargetCatalogStats::default()
        };
        for (solve_ordinal, (outcome, locator)) in
            self.targets.iter().zip(plan.targets()).enumerate()
        {
            if locator.solve_ordinal() != solve_ordinal
                || outcome.locator() != *locator
                || !Arc::ptr_eq(outcome.plan(), plan)
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }
            let handle = target_cases
                .target(locator.inventory_position(), self.limits.target_handle)
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetResolution)?;
            if handle.case_ordinal() != locator.case_ordinal()
                || handle.group_ordinal() != self.group_ordinal
                || handle.ordinal_within_group() != locator.inventory_position()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }
            let authenticated = plan
                .authority()
                .authenticated_same_group_target_case_view(
                    family,
                    context,
                    handle,
                    self.limits.target_case,
                )
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetResolution)?;
            let target_record = authenticated.target();
            if target_record.ordinal() != locator.case_ordinal()
                || target_record.group_ordinal() != self.group_ordinal
                || target_record.ordinal_within_group() != locator.inventory_position()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
            }
            let authority = outcome.authority();
            if authority.case_ordinal() != locator.case_ordinal()
                || authority.group_ordinal() != self.group_ordinal
                || !authority.same_inventory_allocation_as(plan.authority())
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::TargetAuthority);
            }
            authority
                .replay(family, context)
                .map_err(|_| GeneratedAffineResidualGroupExactTargetError::TargetAuthority)?;
            match outcome {
                GeneratedAffineResidualGroupExactTargetOutcome::Ready(target) => {
                    if target.domain.case_ordinal() != locator.case_ordinal()
                        || target.domain.group_ordinal() != self.group_ordinal
                        || !target.domain.same_authority_allocation(authority)
                    {
                        return Err(GeneratedAffineResidualGroupExactTargetError::Premises);
                    }
                    target
                        .domain
                        .replay(family, context, authority)
                        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
                    stats.ready_targets = checked_add(
                        "exact ready targets",
                        stats.ready_targets,
                        1,
                    )?;
                }
                GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                    target,
                ) => {
                    if target.refinement.case_ordinal() != locator.case_ordinal()
                        || target.refinement.group_ordinal() != self.group_ordinal
                        || !target.refinement.same_authority_allocation(authority)
                    {
                        return Err(GeneratedAffineResidualGroupExactTargetError::Premises);
                    }
                    target
                        .refinement
                        .replay(family, context, authority)
                        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
                    stats.equality_refinement_targets = checked_add(
                        "exact equality-refinement targets",
                        stats.equality_refinement_targets,
                        1,
                    )?;
                }
            }
        }
        stats.owner_retained_byte_envelope =
            catalog_retained_bytes(&self.targets, self.targets.capacity(), None)?;
        stats.peak_staging_byte_envelope =
            catalog_peak_staging_bytes(&self.targets, self.targets.capacity(), None)?;
        validate_catalog_stats(self.source_kind, stats, self.limits)?;
        if stats != self.stats {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        Ok(())
    }

    fn replay_direct_formula_singleton_after_plan_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
        let [locator] = plan.targets() else {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        };
        let [outcome] = self.targets.as_slice() else {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        };
        if self.group_ordinal != 0
            || plan.group_ordinal() != 0
            || plan.anchor_case_ordinal() != 0
            || locator.solve_ordinal() != 0
            || locator.inventory_position() != 0
            || locator.case_ordinal() != 0
            || outcome.locator() != *locator
            || !Arc::ptr_eq(outcome.plan(), plan)
            || !Arc::ptr_eq(outcome.authority(), plan.authority())
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
        }
        let authority = outcome.authority();
        if authority.source_kind() != self.source_kind
            || !is_singleton_source_kind(self.source_kind)
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::TargetAuthority);
        }
        match outcome {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(target) => {
                if target.domain.case_ordinal() != 0
                    || target.domain.group_ordinal() != 0
                    || !target.domain.same_authority_allocation(authority)
                {
                    return Err(GeneratedAffineResidualGroupExactTargetError::Premises);
                }
                target
                    .domain
                    .replay(family, context, authority)
                    .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
            }
            GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                target,
            ) => {
                if target.refinement.case_ordinal() != 0
                    || target.refinement.group_ordinal() != 0
                    || !target.refinement.same_authority_allocation(authority)
                {
                    return Err(GeneratedAffineResidualGroupExactTargetError::Premises);
                }
                target
                    .refinement
                    .replay(family, context, authority)
                    .map_err(|_| GeneratedAffineResidualGroupExactTargetError::Premises)?;
            }
        }
        let mut stats = GeneratedAffineResidualGroupExactTargetCatalogStats {
            plan_replays: 1,
            same_group_target_collections: 0,
            targets: 1,
            locator_comparisons: DIRECT_TARGET_LOCATOR_COMPARISONS,
            target_handle_resolutions: 0,
            target_case_authentications: 0,
            target_authority_constructions: 0,
            premises_compilations: 1,
            ready_targets: usize::from(matches!(
                outcome,
                GeneratedAffineResidualGroupExactTargetOutcome::Ready(_)
            )),
            equality_refinement_targets: usize::from(matches!(
                outcome,
                GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(_)
            )),
            retained_plan_references: 2,
            retained_authority_references: 2,
            owner_retained_byte_envelope: catalog_retained_bytes(
                &self.targets,
                self.targets.capacity(),
                None,
            )?,
            peak_staging_byte_envelope: catalog_peak_staging_bytes(
                &[],
                self.targets.capacity(),
                Some(outcome),
            )?
            .max(catalog_peak_staging_bytes(
                &self.targets,
                self.targets.capacity(),
                None,
            )?),
        };
        stats.peak_staging_byte_envelope = stats
            .peak_staging_byte_envelope
            .max(stats.owner_retained_byte_envelope);
        validate_catalog_stats(self.source_kind, stats, self.limits)?;
        if stats != self.stats {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExactTargetDisposition {
    Unresolved,
    Consumed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExactTargetStateOrigin {
    Initial,
    Successor {
        predecessor_state_version: usize,
        predecessor_state_retained_byte_envelope: usize,
        consumed_solve_ordinal: Option<usize>,
    },
}

/// Bounds the checks charged to one target-state transition construction.
/// Authenticated replay is an independently bounded suboperation; its own
/// validation comparisons are not added to this construction ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactTargetStateLimits {
    pub(crate) max_catalog_replays: usize,
    pub(crate) max_database_allocation_comparisons: usize,
    pub(crate) max_predecessor_transition_comparisons: usize,
    pub(crate) max_plan_allocation_comparisons: usize,
    pub(crate) max_frame_allocation_comparisons: usize,
    pub(crate) max_source_state_allocation_comparisons: usize,
    pub(crate) max_group_comparisons: usize,
    pub(crate) max_database_epoch_comparisons: usize,
    pub(crate) max_state_version_comparisons: usize,
    pub(crate) max_disposition_copies: usize,
    pub(crate) max_target_consumptions: usize,
    pub(crate) max_dispositions: usize,
    pub(crate) max_state_retained_byte_envelope: usize,
    pub(crate) max_combined_retained_byte_envelope: usize,
    pub(crate) max_successor_peak_retained_byte_envelope: usize,
}

impl Default for GeneratedAffineResidualGroupExactTargetStateLimits {
    fn default() -> Self {
        Self {
            max_catalog_replays: 1,
            max_database_allocation_comparisons: 2,
            max_predecessor_transition_comparisons: 1,
            max_plan_allocation_comparisons: 1,
            max_frame_allocation_comparisons: 1,
            max_source_state_allocation_comparisons: 1,
            max_group_comparisons: 3,
            max_database_epoch_comparisons: 1,
            max_state_version_comparisons: 2,
            max_disposition_copies: 256_000_000,
            max_target_consumptions: 1,
            max_dispositions: 256_000_000,
            max_state_retained_byte_envelope: usize::MAX / 4,
            max_combined_retained_byte_envelope: usize::MAX / 2,
            max_successor_peak_retained_byte_envelope: usize::MAX / 2,
        }
    }
}

/// Historical construction ledger for a sealed target-state allocation.
/// Replay reconstructs this ledger; its own validation comparisons are not
/// accumulated into the persisted construction statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactTargetStateStats {
    catalog_replays: usize,
    database_allocation_comparisons: usize,
    predecessor_transition_comparisons: usize,
    plan_allocation_comparisons: usize,
    frame_allocation_comparisons: usize,
    source_state_allocation_comparisons: usize,
    group_comparisons: usize,
    database_epoch_comparisons: usize,
    state_version_comparisons: usize,
    disposition_copies: usize,
    target_consumptions: usize,
    dispositions: usize,
    unresolved: usize,
    consumed: usize,
    state_retained_byte_envelope: usize,
    combined_retained_byte_envelope: usize,
    successor_peak_retained_byte_envelope: usize,
}

impl GeneratedAffineResidualGroupExactTargetStateStats {
    pub(crate) const fn dispositions(self) -> usize {
        self.dispositions
    }
    pub(crate) const fn unresolved(self) -> usize {
        self.unresolved
    }
    pub(crate) const fn consumed(self) -> usize {
        self.consumed
    }
    pub(crate) const fn disposition_copies(self) -> usize {
        self.disposition_copies
    }
    pub(crate) const fn target_consumptions(self) -> usize {
        self.target_consumptions
    }
    pub(crate) const fn group_comparisons(self) -> usize {
        self.group_comparisons
    }
    pub(crate) const fn predecessor_transition_comparisons(self) -> usize {
        self.predecessor_transition_comparisons
    }
    pub(crate) const fn state_retained_byte_envelope(self) -> usize {
        self.state_retained_byte_envelope
    }
    pub(crate) const fn combined_retained_byte_envelope(self) -> usize {
        self.combined_retained_byte_envelope
    }
    pub(crate) const fn successor_peak_retained_byte_envelope(self) -> usize {
        self.successor_peak_retained_byte_envelope
    }
}

/// Private inert owner for one catalog's target dispositions.
pub(crate) struct GeneratedAffineResidualGroupExactTargetState {
    schema: &'static str,
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    allocation_nonce: u64,
    binding: GeneratedAffineResidualGroupExactTargetStateBinding,
    catalog: Arc<GeneratedAffineResidualGroupExactTargetCatalog>,
    origin: ExactTargetStateOrigin,
    group_ordinal: usize,
    database_epoch: usize,
    state_version: usize,
    dispositions: Vec<ExactTargetDisposition>,
    limits: GeneratedAffineResidualGroupExactTargetStateLimits,
    stats: GeneratedAffineResidualGroupExactTargetStateStats,
}

/// Sealed preparation that inseparably pairs one unconsumed successor-state
/// allocation with the equality-refinement handle rebound to that exact Arc.
///
/// Only the allocation-sealed session can decompose this value, and it can do
/// so only by presenting its unforgeable database capability immediately
/// before the paired database commit.
pub(crate) struct GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor {
    successor: Arc<GeneratedAffineResidualGroupExactTargetState>,
    target: GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
}

impl GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor {
    pub(crate) fn into_parts_for_session(
        self,
        _capability: &GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    ) -> (
        Arc<GeneratedAffineResidualGroupExactTargetState>,
        GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    ) {
        (self.successor, self.target)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct(
                "GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor",
            )
            .field("state_version", &self.successor.state_version)
            .field("target_solve_ordinal", &self.target.solve_ordinal)
            .field("private_successor", &"<redacted>")
            .field("private_target", &"<redacted>")
            .finish()
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactTargetState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactTargetState")
            .field("schema", &self.schema)
            .field("group_ordinal", &self.group_ordinal)
            .field("database_epoch", &self.database_epoch)
            .field("state_version", &self.state_version)
            .field("target_count", &self.dispositions.len())
            .field("private_allocation_nonce", &"<redacted>")
            .field("private_database_binding", &"<redacted>")
            .field("private_catalog", &"<redacted>")
            .field("private_origin", &"<redacted>")
            .field("private_dispositions", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualGroupExactTargetState {
    /// Construct the inert initial state from the exact database's sealed
    /// allocation authority.  Raw epoch/group/version scalars are deliberately
    /// not accepted: the non-`Clone` binding retains the hidden database nonce
    /// and exact plan/frame allocations needed for a later joint handshake.
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        catalog: Arc<GeneratedAffineResidualGroupExactTargetCatalog>,
        binding: GeneratedAffineResidualGroupExactTargetStateBinding,
        limits: GeneratedAffineResidualGroupExactTargetStateLimits,
    ) -> Result<Arc<Self>, GeneratedAffineResidualGroupExactTargetError> {
        catch_unwind(AssertUnwindSafe(|| {
            for (resource, requested, limit) in [
                (
                    "exact target catalog replays",
                    1,
                    limits.max_catalog_replays,
                ),
                (
                    "exact target database allocation comparisons",
                    0,
                    limits.max_database_allocation_comparisons,
                ),
                (
                    "exact target predecessor transition comparisons",
                    0,
                    limits.max_predecessor_transition_comparisons,
                ),
                (
                    "exact target plan allocation comparisons",
                    1,
                    limits.max_plan_allocation_comparisons,
                ),
                (
                    "exact target frame allocation comparisons",
                    1,
                    limits.max_frame_allocation_comparisons,
                ),
                (
                    "exact target source-state allocation comparisons",
                    0,
                    limits.max_source_state_allocation_comparisons,
                ),
                (
                    "exact target group comparisons",
                    2,
                    limits.max_group_comparisons,
                ),
                (
                    "exact target database epoch comparisons",
                    0,
                    limits.max_database_epoch_comparisons,
                ),
                (
                    "exact target state version comparisons",
                    1,
                    limits.max_state_version_comparisons,
                ),
            ] {
                check_limit(resource, requested, limit)?;
            }
            check_limit(
                "exact target dispositions",
                catalog.len(),
                limits.max_dispositions,
            )?;
            if !binding.same_plan_allocation(&catalog.plan) {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation);
            }
            if !binding.same_frame_allocation(catalog.plan.physical_frame()) {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongFrameAllocation);
            }
            if binding.group_ordinal() != catalog.group_ordinal
                || binding.group_ordinal() != catalog.plan.group_ordinal()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongGroup);
            }
            if binding.state_version() != 0 {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongStateVersion);
            }
            let database_epoch = binding.database_epoch();
            let state_version = binding.state_version();
            // Bound the requested-capacity envelope before retaining the
            // disposition buffer. Observed-capacity checks remain below in
            // case the allocator returns more capacity than requested.
            let prospective_state_retained_byte_envelope = state_retained_bytes(catalog.len())?;
            check_limit(
                "exact target state retained byte envelope",
                prospective_state_retained_byte_envelope,
                limits.max_state_retained_byte_envelope,
            )?;
            let catalog_retained_byte_envelope = catalog_arc_deep_retained_bytes(&catalog)?;
            let prospective_combined_retained_byte_envelope = checked_add(
                "exact target combined retained byte envelope",
                prospective_state_retained_byte_envelope,
                catalog_retained_byte_envelope,
            )?;
            check_limit(
                "exact target combined retained byte envelope",
                prospective_combined_retained_byte_envelope,
                limits.max_combined_retained_byte_envelope,
            )?;
            catalog.replay(family, context, &catalog.plan)?;
            let mut dispositions =
                try_vec_with_capacity("exact target dispositions", catalog.len())?;
            dispositions.resize(catalog.len(), ExactTargetDisposition::Unresolved);
            let state_retained_byte_envelope = state_retained_bytes(dispositions.capacity())?;
            check_limit(
                "exact target state retained byte envelope",
                state_retained_byte_envelope,
                limits.max_state_retained_byte_envelope,
            )?;
            let combined_retained_byte_envelope = checked_add(
                "exact target combined retained byte envelope",
                state_retained_byte_envelope,
                catalog_retained_byte_envelope,
            )?;
            check_limit(
                "exact target combined retained byte envelope",
                combined_retained_byte_envelope,
                limits.max_combined_retained_byte_envelope,
            )?;
            let allocation_nonce = next_exact_target_state_nonce()?;
            let stats = GeneratedAffineResidualGroupExactTargetStateStats {
                catalog_replays: 1,
                database_allocation_comparisons: 0,
                predecessor_transition_comparisons: 0,
                plan_allocation_comparisons: 1,
                frame_allocation_comparisons: 1,
                source_state_allocation_comparisons: 0,
                group_comparisons: 2,
                database_epoch_comparisons: 0,
                state_version_comparisons: 1,
                disposition_copies: 0,
                target_consumptions: 0,
                dispositions: dispositions.len(),
                unresolved: dispositions.len(),
                consumed: 0,
                state_retained_byte_envelope,
                combined_retained_byte_envelope,
                successor_peak_retained_byte_envelope: 0,
            };
            let source_kind = catalog.source_kind;
            Ok(Arc::new(Self {
                schema: exact_target_state_schema_for_source(source_kind),
                source_kind,
                allocation_nonce,
                binding,
                group_ordinal: catalog.group_ordinal,
                catalog,
                origin: ExactTargetStateOrigin::Initial,
                database_epoch,
                state_version,
                dispositions,
                limits,
                stats,
            }))
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.source_kind
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn database_epoch(&self) -> usize {
        self.database_epoch
    }
    pub(crate) const fn state_version(&self) -> usize {
        self.state_version
    }
    /// Sealed database authority for a joint state/database authentication.
    /// The hidden nonce remains inaccessible; callers may only borrow and
    /// present this value back to the exact database that minted it.
    pub(crate) const fn binding(&self) -> &GeneratedAffineResidualGroupExactTargetStateBinding {
        &self.binding
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactTargetStateLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactTargetStateStats {
        self.stats
    }

    /// Allocation identity seam for the transactional session owner. The
    /// caller supplies an owning state handle, never a scalar nonce.
    pub(crate) fn same_allocation(self: &Arc<Self>, other: &Arc<Self>) -> bool {
        Arc::ptr_eq(self, other)
    }

    /// Prepare an immutable target-state successor for one authenticated
    /// database transition.  This method mutates neither the source state nor
    /// the database.  A Ready handle may consume exactly its one unresolved
    /// target; equality-refinement handles are excluded by the type system.
    pub(crate) fn prepare_successor(
        self: &Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        binding: GeneratedAffineResidualGroupExactTargetStateBinding,
        consume: Option<GeneratedAffineResidualGroupRetainedReadyExactTarget>,
    ) -> Result<Arc<Self>, GeneratedAffineResidualGroupExactTargetError> {
        catch_unwind(AssertUnwindSafe(|| {
            let state_version = self
                .state_version
                .checked_add(1)
                .ok_or(GeneratedAffineResidualGroupExactTargetError::StateVersionOverflow)?;
            if !binding.same_database_allocation(&self.binding) {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongDatabaseAllocation);
            }
            if !binding.same_plan_allocation(&self.catalog.plan) {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation);
            }
            if !binding.same_frame_allocation(self.catalog.plan.physical_frame()) {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongFrameAllocation);
            }
            if binding.group_ordinal() != self.group_ordinal
                || binding.group_ordinal() != self.catalog.group_ordinal
                || binding.group_ordinal() != self.catalog.plan.group_ordinal()
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongGroup);
            }
            if binding.database_epoch() != self.database_epoch {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongDatabaseEpoch);
            }
            if binding.state_version() != state_version {
                return Err(GeneratedAffineResidualGroupExactTargetError::WrongStateVersion);
            }
            if !binding.is_direct_successor_of(&self.binding) {
                return Err(
                    GeneratedAffineResidualGroupExactTargetError::WrongPredecessorTransition,
                );
            }
            let consumed_solve_ordinal = if let Some(target) = consume.as_ref() {
                if !target.authenticates_source_state(self) {
                    return Err(
                        GeneratedAffineResidualGroupExactTargetError::WrongSourceStateAllocation,
                    );
                }
                let solve_ordinal = target.solve_ordinal;
                if self.dispositions.get(solve_ordinal) != Some(&ExactTargetDisposition::Unresolved)
                {
                    return Err(if solve_ordinal >= self.dispositions.len() {
                        GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange
                    } else {
                        GeneratedAffineResidualGroupExactTargetError::TargetConsumed
                    });
                }
                if !matches!(
                    self.catalog.targets.get(solve_ordinal),
                    Some(GeneratedAffineResidualGroupExactTargetOutcome::Ready(_))
                ) {
                    return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
                }
                Some(solve_ordinal)
            } else {
                None
            };
            self.replay_unwind_boundary(
                family,
                context,
                &self.catalog.plan,
                self.group_ordinal,
                self.database_epoch,
                self.state_version,
            )?;
            self.prepare_successor_copy_tail(binding, consumed_solve_ordinal, state_version, 1)
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    /// Prepare the publication successor from owners that were already
    /// algebraically sealed before the live commit boundary.  This path does
    /// no catalog/Symbolica replay; it performs only checked resource work,
    /// allocation-bound comparisons, and the successor allocation itself.
    pub(crate) fn prepare_publication_successor(
        self: &Arc<Self>,
        binding: GeneratedAffineResidualGroupExactTargetStateBinding,
        consume: &GeneratedAffineResidualGroupRetainedReadyExactTarget,
    ) -> Result<Arc<Self>, GeneratedAffineResidualGroupExactTargetError> {
        let state_version = self
            .state_version
            .checked_add(1)
            .ok_or(GeneratedAffineResidualGroupExactTargetError::StateVersionOverflow)?;
        let solve_ordinal = consume.solve_ordinal;
        debug_assert!(binding.is_direct_successor_of(&self.binding));
        debug_assert!(consume.authenticates_source_state(self));
        debug_assert_eq!(
            self.dispositions.get(solve_ordinal),
            Some(&ExactTargetDisposition::Unresolved)
        );
        self.prepare_successor_copy_tail(binding, Some(solve_ordinal), state_version, 0)
    }

    fn prepare_successor_copy_tail(
        self: &Arc<Self>,
        binding: GeneratedAffineResidualGroupExactTargetStateBinding,
        consumed_solve_ordinal: Option<usize>,
        state_version: usize,
        catalog_replays: usize,
    ) -> Result<Arc<Self>, GeneratedAffineResidualGroupExactTargetError> {
        (|| {
            let target_consumptions = usize::from(consumed_solve_ordinal.is_some());
            for (resource, requested, limit) in [
                (
                    "exact target catalog replays",
                    catalog_replays,
                    self.limits.max_catalog_replays,
                ),
                (
                    "exact target database allocation comparisons",
                    2,
                    self.limits.max_database_allocation_comparisons,
                ),
                (
                    "exact target predecessor transition comparisons",
                    1,
                    self.limits.max_predecessor_transition_comparisons,
                ),
                (
                    "exact target plan allocation comparisons",
                    1,
                    self.limits.max_plan_allocation_comparisons,
                ),
                (
                    "exact target frame allocation comparisons",
                    1,
                    self.limits.max_frame_allocation_comparisons,
                ),
                (
                    "exact target source-state allocation comparisons",
                    target_consumptions,
                    self.limits.max_source_state_allocation_comparisons,
                ),
                (
                    "exact target group comparisons",
                    3,
                    self.limits.max_group_comparisons,
                ),
                (
                    "exact target database epoch comparisons",
                    1,
                    self.limits.max_database_epoch_comparisons,
                ),
                (
                    "exact target state version comparisons",
                    2,
                    self.limits.max_state_version_comparisons,
                ),
                (
                    "exact target disposition copies",
                    self.dispositions.len(),
                    self.limits.max_disposition_copies,
                ),
                (
                    "exact target consumptions",
                    target_consumptions,
                    self.limits.max_target_consumptions,
                ),
            ] {
                check_limit(resource, requested, limit)?;
            }

            // Reject a successor whose minimum requested-capacity envelope is
            // already out of policy before retaining its copied disposition
            // buffer.  The observed-capacity checks below remain necessary:
            // an allocator may return more capacity than was requested.
            let prospective_state_retained_byte_envelope =
                state_retained_bytes(self.dispositions.len())?;
            check_limit(
                "exact target state retained byte envelope",
                prospective_state_retained_byte_envelope,
                self.limits.max_state_retained_byte_envelope,
            )?;
            let catalog_retained_byte_envelope = catalog_arc_deep_retained_bytes(&self.catalog)?;
            let prospective_combined_retained_byte_envelope = checked_add(
                "exact target combined retained byte envelope",
                prospective_state_retained_byte_envelope,
                catalog_retained_byte_envelope,
            )?;
            check_limit(
                "exact target combined retained byte envelope",
                prospective_combined_retained_byte_envelope,
                self.limits.max_combined_retained_byte_envelope,
            )?;
            let predecessor_state_retained_byte_envelope = self.stats.state_retained_byte_envelope;
            let prospective_successor_peak_retained_byte_envelope = checked_sum(
                "exact target successor peak retained byte envelope",
                [
                    predecessor_state_retained_byte_envelope,
                    prospective_state_retained_byte_envelope,
                    catalog_retained_byte_envelope,
                ],
            )?;
            check_limit(
                "exact target successor peak retained byte envelope",
                prospective_successor_peak_retained_byte_envelope,
                self.limits.max_successor_peak_retained_byte_envelope,
            )?;

            let mut dispositions = try_vec_with_capacity(
                "exact target successor dispositions",
                self.dispositions.len(),
            )?;
            dispositions.extend_from_slice(&self.dispositions);
            if let Some(solve_ordinal) = consumed_solve_ordinal {
                dispositions[solve_ordinal] = ExactTargetDisposition::Consumed;
            }
            let unresolved = dispositions
                .iter()
                .filter(|&&value| value == ExactTargetDisposition::Unresolved)
                .count();
            let consumed = dispositions.len() - unresolved;
            let state_retained_byte_envelope = state_retained_bytes(dispositions.capacity())?;
            check_limit(
                "exact target state retained byte envelope",
                state_retained_byte_envelope,
                self.limits.max_state_retained_byte_envelope,
            )?;
            let combined_retained_byte_envelope = checked_add(
                "exact target combined retained byte envelope",
                state_retained_byte_envelope,
                catalog_retained_byte_envelope,
            )?;
            check_limit(
                "exact target combined retained byte envelope",
                combined_retained_byte_envelope,
                self.limits.max_combined_retained_byte_envelope,
            )?;
            let successor_peak_retained_byte_envelope = checked_sum(
                "exact target successor peak retained byte envelope",
                [
                    predecessor_state_retained_byte_envelope,
                    state_retained_byte_envelope,
                    catalog_retained_byte_envelope,
                ],
            )?;
            check_limit(
                "exact target successor peak retained byte envelope",
                successor_peak_retained_byte_envelope,
                self.limits.max_successor_peak_retained_byte_envelope,
            )?;
            let allocation_nonce = next_exact_target_state_nonce()?;
            let stats = GeneratedAffineResidualGroupExactTargetStateStats {
                catalog_replays,
                database_allocation_comparisons: 2,
                predecessor_transition_comparisons: 1,
                plan_allocation_comparisons: 1,
                frame_allocation_comparisons: 1,
                source_state_allocation_comparisons: target_consumptions,
                group_comparisons: 3,
                database_epoch_comparisons: 1,
                state_version_comparisons: 2,
                disposition_copies: dispositions.len(),
                target_consumptions,
                dispositions: dispositions.len(),
                unresolved,
                consumed,
                state_retained_byte_envelope,
                combined_retained_byte_envelope,
                successor_peak_retained_byte_envelope,
            };
            Ok(Arc::new(Self {
                schema: exact_target_state_schema_for_source(self.source_kind),
                source_kind: self.source_kind,
                allocation_nonce,
                binding,
                catalog: Arc::clone(&self.catalog),
                origin: ExactTargetStateOrigin::Successor {
                    predecessor_state_version: self.state_version,
                    predecessor_state_retained_byte_envelope,
                    consumed_solve_ordinal,
                },
                group_ordinal: self.group_ordinal,
                database_epoch: self.database_epoch,
                state_version,
                dispositions,
                limits: self.limits,
                stats,
            }))
        })()
    }

    /// Prepare one unconsumed successor together with an equality target
    /// retained from that exact successor allocation.
    ///
    /// The target cannot be rebound after the fact: both fields are minted in
    /// this module from the same newly allocated Arc and leave only as one
    /// capability-gated preparation.
    pub(crate) fn prepare_equality_refinement_successor(
        self: &Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        binding: GeneratedAffineResidualGroupExactTargetStateBinding,
        target: &GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    ) -> Result<
        GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor,
        GeneratedAffineResidualGroupExactTargetError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            check_limit(
                "exact target source-state allocation comparisons",
                1,
                self.limits.max_source_state_allocation_comparisons,
            )?;
            if !target.authenticates_source_state(self) {
                return Err(
                    GeneratedAffineResidualGroupExactTargetError::WrongSourceStateAllocation,
                );
            }
            let solve_ordinal = target.solve_ordinal;
            if self.dispositions.get(solve_ordinal) != Some(&ExactTargetDisposition::Unresolved) {
                return Err(if solve_ordinal >= self.dispositions.len() {
                    GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange
                } else {
                    GeneratedAffineResidualGroupExactTargetError::TargetConsumed
                });
            }
            if !matches!(
                self.catalog.targets.get(solve_ordinal),
                Some(
                    GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                        _,
                    )
                )
            ) {
                return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
            }
            let successor = self.prepare_successor(family, context, binding, None)?;
            let target = GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget {
                state: Arc::clone(&successor),
                solve_ordinal,
            };
            Ok(
                GeneratedAffineResidualGroupPreparedEqualityRefinementExactTargetSuccessor {
                    successor,
                    target,
                },
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        group_ordinal: usize,
        database_epoch: usize,
        state_version: usize,
    ) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.replay_unwind_boundary(
                family,
                context,
                plan,
                group_ordinal,
                database_epoch,
                state_version,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::SymbolicaPanic)?
    }

    fn replay_unwind_boundary(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        group_ordinal: usize,
        database_epoch: usize,
        state_version: usize,
    ) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
        // Reconstruct the historical construction ledger from the sealed
        // origin. The validation comparisons performed below authenticate
        // replay itself and are not accumulated into that ledger.
        let (
            catalog_replays,
            database_allocation_comparisons,
            predecessor_transition_comparisons,
            group_comparisons,
            database_epoch_comparisons,
            state_version_comparisons,
            source_state_allocation_comparisons,
            disposition_copies,
            target_consumptions,
        ) = match self.origin {
            ExactTargetStateOrigin::Initial => (1, 0, 0, 2, 0, 1, 0, 0, 0),
            ExactTargetStateOrigin::Successor {
                consumed_solve_ordinal,
                ..
            } => (
                self.stats.catalog_replays,
                2,
                1,
                3,
                1,
                2,
                usize::from(consumed_solve_ordinal.is_some()),
                self.dispositions.len(),
                usize::from(consumed_solve_ordinal.is_some()),
            ),
        };
        for (resource, requested, limit) in [
            (
                "exact target catalog replays",
                catalog_replays,
                self.limits.max_catalog_replays,
            ),
            (
                "exact target database allocation comparisons",
                database_allocation_comparisons,
                self.limits.max_database_allocation_comparisons,
            ),
            (
                "exact target predecessor transition comparisons",
                predecessor_transition_comparisons,
                self.limits.max_predecessor_transition_comparisons,
            ),
            (
                "exact target plan allocation comparisons",
                1,
                self.limits.max_plan_allocation_comparisons,
            ),
            (
                "exact target frame allocation comparisons",
                1,
                self.limits.max_frame_allocation_comparisons,
            ),
            (
                "exact target source-state allocation comparisons",
                source_state_allocation_comparisons,
                self.limits.max_source_state_allocation_comparisons,
            ),
            (
                "exact target group comparisons",
                group_comparisons,
                self.limits.max_group_comparisons,
            ),
            (
                "exact target database epoch comparisons",
                database_epoch_comparisons,
                self.limits.max_database_epoch_comparisons,
            ),
            (
                "exact target state version comparisons",
                state_version_comparisons,
                self.limits.max_state_version_comparisons,
            ),
            (
                "exact target disposition copies",
                disposition_copies,
                self.limits.max_disposition_copies,
            ),
            (
                "exact target consumptions",
                target_consumptions,
                self.limits.max_target_consumptions,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        if self.source_kind != self.catalog.source_kind
            || self.source_kind != plan.source_kind()
            || self.schema != exact_target_state_schema_for_source(self.source_kind)
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        if self.allocation_nonce == 0 {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        if !self.catalog.same_plan_allocation(plan) {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation);
        }
        if !self.binding.same_plan_allocation(plan) {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation);
        }
        if !self.binding.same_frame_allocation(plan.physical_frame()) {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongFrameAllocation);
        }
        if self.group_ordinal != group_ordinal
            || self.group_ordinal != plan.group_ordinal()
            || self.catalog.group_ordinal != self.group_ordinal
            || self.binding.group_ordinal() != self.group_ordinal
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongGroup);
        }
        if self.database_epoch != database_epoch
            || self.binding.database_epoch() != self.database_epoch
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongDatabaseEpoch);
        }
        if self.state_version != state_version || self.binding.state_version() != self.state_version
        {
            return Err(GeneratedAffineResidualGroupExactTargetError::WrongStateVersion);
        }
        match self.origin {
            ExactTargetStateOrigin::Initial => {
                if self.state_version != 0 {
                    return Err(GeneratedAffineResidualGroupExactTargetError::WrongStateVersion);
                }
            }
            ExactTargetStateOrigin::Successor {
                predecessor_state_version,
                ..
            } => {
                if predecessor_state_version.checked_add(1) != Some(self.state_version) {
                    return Err(GeneratedAffineResidualGroupExactTargetError::WrongStateVersion);
                }
            }
        }
        check_limit(
            "exact target dispositions",
            self.dispositions.len(),
            self.limits.max_dispositions,
        )?;
        if self.dispositions.len() != self.catalog.len() {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        if let ExactTargetStateOrigin::Successor {
            consumed_solve_ordinal: Some(solve_ordinal),
            ..
        } = self.origin
        {
            if self.dispositions.get(solve_ordinal) != Some(&ExactTargetDisposition::Consumed)
                || !matches!(
                    self.catalog.targets.get(solve_ordinal),
                    Some(GeneratedAffineResidualGroupExactTargetOutcome::Ready(_))
                )
            {
                return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
            }
        }
        self.catalog.replay(family, context, plan)?;
        let unresolved = self
            .dispositions
            .iter()
            .filter(|&&value| value == ExactTargetDisposition::Unresolved)
            .count();
        let consumed = self.dispositions.len() - unresolved;
        let state_retained_byte_envelope = state_retained_bytes(self.dispositions.capacity())?;
        let combined_retained_byte_envelope = checked_add(
            "exact target combined retained byte envelope",
            state_retained_byte_envelope,
            catalog_arc_deep_retained_bytes(&self.catalog)?,
        )?;
        let successor_peak_retained_byte_envelope = match self.origin {
            ExactTargetStateOrigin::Initial => 0,
            ExactTargetStateOrigin::Successor {
                predecessor_state_retained_byte_envelope,
                ..
            } => checked_sum(
                "exact target successor peak retained byte envelope",
                [
                    predecessor_state_retained_byte_envelope,
                    state_retained_byte_envelope,
                    catalog_arc_deep_retained_bytes(&self.catalog)?,
                ],
            )?,
        };
        check_limit(
            "exact target state retained byte envelope",
            state_retained_byte_envelope,
            self.limits.max_state_retained_byte_envelope,
        )?;
        check_limit(
            "exact target combined retained byte envelope",
            combined_retained_byte_envelope,
            self.limits.max_combined_retained_byte_envelope,
        )?;
        check_limit(
            "exact target successor peak retained byte envelope",
            successor_peak_retained_byte_envelope,
            self.limits.max_successor_peak_retained_byte_envelope,
        )?;
        let stats = GeneratedAffineResidualGroupExactTargetStateStats {
            catalog_replays,
            database_allocation_comparisons,
            predecessor_transition_comparisons,
            plan_allocation_comparisons: 1,
            frame_allocation_comparisons: 1,
            source_state_allocation_comparisons,
            group_comparisons,
            database_epoch_comparisons,
            state_version_comparisons,
            disposition_copies,
            target_consumptions,
            dispositions: self.dispositions.len(),
            unresolved,
            consumed,
            state_retained_byte_envelope,
            combined_retained_byte_envelope,
            successor_peak_retained_byte_envelope,
        };
        if stats != self.stats {
            return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
        }
        Ok(())
    }

    pub(crate) fn authenticated_view<'state>(
        self: &'state Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualGroupExactTargetStateView<'state>,
        GeneratedAffineResidualGroupExactTargetError,
    > {
        self.replay(
            family,
            context,
            &self.catalog.plan,
            self.group_ordinal,
            self.database_epoch,
            self.state_version,
        )?;
        Ok(GeneratedAffineResidualGroupExactTargetStateView { state: self })
    }
}

/// Sealed borrow minted only after exact plan/group/epoch/version replay.
pub(crate) struct GeneratedAffineResidualGroupExactTargetStateView<'state> {
    state: &'state Arc<GeneratedAffineResidualGroupExactTargetState>,
}

impl<'state> GeneratedAffineResidualGroupExactTargetStateView<'state> {
    /// Borrow the same sealed authority retained by the state so the exact
    /// database can jointly authenticate allocation and version.
    pub(crate) fn binding(&self) -> &GeneratedAffineResidualGroupExactTargetStateBinding {
        self.state.binding()
    }

    /// Exact state-allocation check for the sealed DB/session handshake. No
    /// scalar identity is accepted or revealed.
    pub(crate) fn authenticates_state_allocation(
        &self,
        state: &Arc<GeneratedAffineResidualGroupExactTargetState>,
    ) -> bool {
        Arc::ptr_eq(self.state, state)
    }

    /// Persisted solve ordinals in their exact plan order.
    pub(crate) fn iter(&self) -> Range<usize> {
        0..self.state.dispositions.len()
    }

    pub(crate) fn is_unresolved(
        &self,
        solve_ordinal: usize,
    ) -> Result<bool, GeneratedAffineResidualGroupExactTargetError> {
        self.state
            .dispositions
            .get(solve_ordinal)
            .map(|&value| value == ExactTargetDisposition::Unresolved)
            .ok_or(GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange)
    }

    pub(crate) fn authenticated_target(
        &self,
        solve_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualGroupAuthenticatedExactTargetView<'state>,
        GeneratedAffineResidualGroupExactTargetError,
    > {
        if !self.is_unresolved(solve_ordinal)? {
            return Err(GeneratedAffineResidualGroupExactTargetError::TargetConsumed);
        }
        match self.state.catalog.targets.get(solve_ordinal).ok_or(
            GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange,
        )? {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(target) => Ok(
                GeneratedAffineResidualGroupAuthenticatedExactTargetView::Ready(
                    GeneratedAffineResidualGroupReadyExactTargetView { target },
                ),
            ),
            GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                target,
            ) => Ok(
                GeneratedAffineResidualGroupAuthenticatedExactTargetView::RequiresAffineEqualityRefinement(
                    GeneratedAffineResidualGroupEqualityRefinementExactTargetView { target },
                ),
            ),
        }
    }

    /// Retain one unresolved target as a sealed Arc-owned handle.
    ///
    /// The handle copies no target payload: it keeps the exact authenticated
    /// state allocation alive and resolves its solve ordinal back into that
    /// state's immutable catalog whenever a borrowed view is requested.
    pub(crate) fn retain_target(
        &self,
        solve_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualGroupRetainedExactTarget,
        GeneratedAffineResidualGroupExactTargetError,
    > {
        if !self.is_unresolved(solve_ordinal)? {
            return Err(GeneratedAffineResidualGroupExactTargetError::TargetConsumed);
        }
        match self.state.catalog.targets.get(solve_ordinal).ok_or(
            GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange,
        )? {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(_) => Ok(
                GeneratedAffineResidualGroupRetainedExactTarget::Ready(
                    GeneratedAffineResidualGroupRetainedReadyExactTarget {
                        state: Arc::clone(self.state),
                        solve_ordinal,
                    },
                ),
            ),
            GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                _,
            ) => Ok(
                GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(
                    GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget {
                        state: Arc::clone(self.state),
                        solve_ordinal,
                    },
                ),
            ),
        }
    }
}

/// Arc-owned, non-`Clone` target retained from one authenticated state view.
/// Equality-bearing targets remain a distinct typed outcome and cannot be
/// passed to the Ready-only successor-consumption seam.
pub(crate) enum GeneratedAffineResidualGroupRetainedExactTarget {
    Ready(GeneratedAffineResidualGroupRetainedReadyExactTarget),
    RequiresAffineEqualityRefinement(
        GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget,
    ),
}

impl GeneratedAffineResidualGroupRetainedExactTarget {
    pub(crate) const fn solve_ordinal(&self) -> usize {
        match self {
            Self::Ready(target) => target.solve_ordinal,
            Self::RequiresAffineEqualityRefinement(target) => target.solve_ordinal,
        }
    }

    pub(crate) fn locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        match self {
            Self::Ready(target) => target.locator(),
            Self::RequiresAffineEqualityRefinement(target) => target.locator(),
        }
    }
}

/// Sealed retained handle for a Ready exact target.
pub(crate) struct GeneratedAffineResidualGroupRetainedReadyExactTarget {
    state: Arc<GeneratedAffineResidualGroupExactTargetState>,
    solve_ordinal: usize,
}

impl GeneratedAffineResidualGroupRetainedReadyExactTarget {
    fn target(&self) -> &GeneratedAffineResidualGroupReadyExactTarget {
        match &self.state.catalog.targets[self.solve_ordinal] {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(target) => target,
            GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(_) => {
                unreachable!("sealed Ready target handle changed outcome")
            }
        }
    }

    pub(crate) const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }

    pub(crate) fn locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        &self.target().locator
    }

    pub(crate) fn domain(&self) -> &GeneratedAffineResidualCasePremisesCertificate {
        &self.target().domain
    }

    pub(crate) fn premises(&self) -> &[ParametricNonZeroCondition] {
        self.domain().premises()
    }

    pub(crate) fn authenticates_source_state(
        &self,
        state: &Arc<GeneratedAffineResidualGroupExactTargetState>,
    ) -> bool {
        Arc::ptr_eq(&self.state, state)
    }
}

/// Sealed retained handle for a target whose affine equality predicates still
/// require typed refinement.
pub(crate) struct GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget {
    state: Arc<GeneratedAffineResidualGroupExactTargetState>,
    solve_ordinal: usize,
}

impl GeneratedAffineResidualGroupRetainedEqualityRefinementExactTarget {
    fn target(&self) -> &GeneratedAffineResidualGroupEqualityRefinementExactTarget {
        match &self.state.catalog.targets[self.solve_ordinal] {
            GeneratedAffineResidualGroupExactTargetOutcome::Ready(_) => {
                unreachable!("sealed equality-refinement target handle changed outcome")
            }
            GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                target,
            ) => target,
        }
    }

    pub(crate) const fn solve_ordinal(&self) -> usize {
        self.solve_ordinal
    }

    pub(crate) fn locator(&self) -> &GeneratedAffineResidualGroupSolveTargetLocator {
        &self.target().locator
    }

    pub(crate) fn refinement(&self) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        &self.target().refinement
    }

    pub(crate) fn authenticates_source_state(
        &self,
        state: &Arc<GeneratedAffineResidualGroupExactTargetState>,
    ) -> bool {
        Arc::ptr_eq(&self.state, state)
    }
}

pub(crate) enum GeneratedAffineResidualGroupAuthenticatedExactTargetView<'target> {
    Ready(GeneratedAffineResidualGroupReadyExactTargetView<'target>),
    RequiresAffineEqualityRefinement(
        GeneratedAffineResidualGroupEqualityRefinementExactTargetView<'target>,
    ),
}

pub(crate) struct GeneratedAffineResidualGroupReadyExactTargetView<'target> {
    target: &'target GeneratedAffineResidualGroupReadyExactTarget,
}

impl GeneratedAffineResidualGroupReadyExactTargetView<'_> {
    pub(crate) const fn locator(&self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.target.locator
    }
    pub(crate) fn case_ordinal(&self) -> usize {
        self.target.authority.case_ordinal()
    }
    pub(crate) fn group_ordinal(&self) -> usize {
        self.target.authority.group_ordinal()
    }
    pub(crate) fn domain(&self) -> &GeneratedAffineResidualCasePremisesCertificate {
        &self.target.domain
    }
    pub(crate) fn premises(&self) -> &[ParametricNonZeroCondition] {
        self.target.domain.premises()
    }
}

pub(crate) struct GeneratedAffineResidualGroupEqualityRefinementExactTargetView<'target> {
    target: &'target GeneratedAffineResidualGroupEqualityRefinementExactTarget,
}

impl GeneratedAffineResidualGroupEqualityRefinementExactTargetView<'_> {
    pub(crate) const fn locator(&self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.target.locator
    }
    pub(crate) fn case_ordinal(&self) -> usize {
        self.target.authority.case_ordinal()
    }
    pub(crate) fn group_ordinal(&self) -> usize {
        self.target.authority.group_ordinal()
    }
    pub(crate) fn refinement(&self) -> &GeneratedAffineResidualCaseEqualityRefinementCertificate {
        &self.target.refinement
    }
}

fn preflight_catalog_counts(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    targets: usize,
    limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
    if is_singleton_source_kind(source_kind) && targets != 1 {
        return Err(GeneratedAffineResidualGroupExactTargetError::MalformedTargetOrder);
    }
    let (
        same_group_target_collections,
        locator_comparisons_per_target,
        target_handle_resolutions,
        target_case_authentications,
        target_authority_constructions,
    ) = match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => {
            (1, TARGET_LOCATOR_COMPARISONS, targets, targets, targets)
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            (0, DIRECT_TARGET_LOCATOR_COMPARISONS, 0, 0, 0)
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            (0, DIRECT_TARGET_LOCATOR_COMPARISONS, 0, 0, 0)
        }
    };
    let locator_comparisons = checked_mul(
        "exact target locator comparisons",
        targets,
        locator_comparisons_per_target,
    )?;
    let plan_references = checked_add("exact target retained plan references", targets, 1)?;
    let authority_references =
        checked_mul("exact target retained authority references", targets, 2)?;
    for (resource, requested, limit) in [
        ("exact target plan replays", 1, limits.max_plan_replays),
        (
            "exact target same-group collections",
            same_group_target_collections,
            limits.max_same_group_target_collections,
        ),
        ("exact targets", targets, limits.max_targets),
        (
            "exact target locator comparisons",
            locator_comparisons,
            limits.max_locator_comparisons,
        ),
        (
            "exact target handle resolutions",
            target_handle_resolutions,
            limits.max_target_handle_resolutions,
        ),
        (
            "exact target case authentications",
            target_case_authentications,
            limits.max_target_case_authentications,
        ),
        (
            "exact target authority constructions",
            target_authority_constructions,
            limits.max_target_authority_constructions,
        ),
        (
            "exact target premise compilations",
            targets,
            limits.max_premises_compilations,
        ),
        (
            "exact target retained plan references",
            plan_references,
            limits.max_retained_plan_references,
        ),
        (
            "exact target retained authority references",
            authority_references,
            limits.max_retained_authority_references,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn validate_catalog_stats(
    source_kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    stats: GeneratedAffineResidualGroupExactTargetCatalogStats,
    limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
    preflight_catalog_counts(source_kind, stats.targets, limits)?;
    let (
        same_group_target_collections,
        locator_comparisons_per_target,
        target_handle_resolutions,
        target_case_authentications,
        target_authority_constructions,
    ) = match source_kind {
        GeneratedAffineResidualCaseAuthoritySourceKind::LegacyInventory => (
            1,
            TARGET_LOCATOR_COMPARISONS,
            stats.targets,
            stats.targets,
            stats.targets,
        ),
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            (0, DIRECT_TARGET_LOCATOR_COMPARISONS, 0, 0, 0)
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            (0, DIRECT_TARGET_LOCATOR_COMPARISONS, 0, 0, 0)
        }
    };
    if stats.plan_replays != 1
        || stats.same_group_target_collections != same_group_target_collections
        || stats.locator_comparisons
            != checked_mul(
                "exact target locator comparisons",
                stats.targets,
                locator_comparisons_per_target,
            )?
        || stats.target_handle_resolutions != target_handle_resolutions
        || stats.target_case_authentications != target_case_authentications
        || stats.target_authority_constructions != target_authority_constructions
        || stats.premises_compilations != stats.targets
        || checked_add(
            "exact target outcome count",
            stats.ready_targets,
            stats.equality_refinement_targets,
        )? != stats.targets
    {
        return Err(GeneratedAffineResidualGroupExactTargetError::ReplayMismatch);
    }
    check_limit(
        "exact ready targets",
        stats.ready_targets,
        limits.max_ready_targets,
    )?;
    check_limit(
        "exact equality-refinement targets",
        stats.equality_refinement_targets,
        limits.max_equality_refinement_targets,
    )?;
    check_limit(
        "exact target owner retained byte envelope",
        stats.owner_retained_byte_envelope,
        limits.max_owner_retained_byte_envelope,
    )?;
    check_limit(
        "exact target peak staging byte envelope",
        stats.peak_staging_byte_envelope,
        limits.max_peak_staging_byte_envelope,
    )
}

fn catalog_base_retained_bytes(
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    checked_add(
        "exact target owner retained byte envelope",
        size_of::<GeneratedAffineResidualGroupExactTargetCatalog>(),
        checked_mul(
            "exact target owner retained byte envelope",
            capacity,
            size_of::<GeneratedAffineResidualGroupExactTargetOutcome>(),
        )?,
    )
}

fn catalog_retained_bytes(
    targets: &[GeneratedAffineResidualGroupExactTargetOutcome],
    capacity: usize,
    candidate: Option<&GeneratedAffineResidualGroupExactTargetOutcome>,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    if capacity
        < checked_add(
            "exact target catalog slots",
            targets.len(),
            usize::from(candidate.is_some()),
        )?
    {
        return Err(
            GeneratedAffineResidualGroupExactTargetError::AllocationFailure {
                resource: "exact target catalog slots",
            },
        );
    }
    let mut retained = catalog_base_retained_bytes(capacity)?;
    for target in targets.iter().chain(candidate) {
        retained = checked_add(
            "exact target owner retained byte envelope",
            retained,
            target_retained_extra(target)?,
        )?;
    }
    Ok(retained)
}

fn target_retained_extra(
    target: &GeneratedAffineResidualGroupExactTargetOutcome,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    let authority_allocation = if is_singleton_source_kind(target.authority().source_kind()) {
        // A singleton target reuses the exact authority allocation
        // already owned by the retained plan. Its inline Arc handle is part
        // of the target enum's structural slot; do not charge the shared
        // pointee a second time.
        0
    } else {
        arc_allocation_byte_envelope::<GeneratedAffineResidualCaseAuthority>()?
    };
    checked_add(
        "exact target owner retained byte envelope",
        authority_allocation,
        target.child_retained_bytes()?,
    )
}

fn catalog_peak_staging_bytes(
    targets: &[GeneratedAffineResidualGroupExactTargetOutcome],
    capacity: usize,
    candidate: Option<&GeneratedAffineResidualGroupExactTargetOutcome>,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    let mut retained = catalog_base_retained_bytes(capacity)?;
    let mut peak = retained;
    for target in targets.iter().chain(candidate) {
        let authority = target.authority();
        let child_peak = target.child_peak_bytes();
        let authority_peak = authority.stats().replay_owned_logical_peak();
        let authority_allocation = if is_singleton_source_kind(authority.source_kind()) {
            0
        } else {
            arc_allocation_byte_envelope::<GeneratedAffineResidualCaseAuthority>()?
        };
        let staged = checked_sum(
            "exact target peak staging byte envelope",
            [
                retained,
                authority_allocation,
                authority_peak.max(child_peak),
            ],
        )?;
        peak = peak.max(staged);
        retained = checked_add(
            "exact target peak staging byte envelope",
            retained,
            target_retained_extra(target)?,
        )?;
    }
    Ok(peak.max(retained))
}

fn state_retained_bytes(
    disposition_capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    checked_add(
        "exact target state retained byte envelope",
        arc_allocation_byte_envelope::<GeneratedAffineResidualGroupExactTargetState>()?,
        checked_mul(
            "exact target state retained byte envelope",
            disposition_capacity,
            size_of::<ExactTargetDisposition>(),
        )?,
    )
}

fn catalog_arc_deep_retained_bytes(
    catalog: &GeneratedAffineResidualGroupExactTargetCatalog,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    let control_and_padding =
        arc_allocation_byte_envelope::<GeneratedAffineResidualGroupExactTargetCatalog>()?
            .checked_sub(size_of::<GeneratedAffineResidualGroupExactTargetCatalog>())
            .ok_or(
                GeneratedAffineResidualGroupExactTargetError::ResourceCountOverflow {
                    resource: "exact target catalog Arc bytes",
                },
            )?;
    checked_add(
        "exact target catalog Arc bytes",
        control_and_padding,
        catalog.stats.owner_retained_byte_envelope,
    )
}

fn arc_allocation_byte_envelope<T>() -> Result<usize, GeneratedAffineResidualGroupExactTargetError>
{
    checked_sum(
        "exact target Arc allocation bytes",
        [
            checked_mul(
                "exact target Arc allocation bytes",
                2,
                size_of::<AtomicUsize>(),
            )?,
            checked_mul(
                "exact target Arc allocation bytes",
                2,
                align_of::<T>().saturating_sub(1),
            )?,
            size_of::<T>(),
        ],
    )
}

fn next_exact_target_state_nonce() -> Result<u64, GeneratedAffineResidualGroupExactTargetError> {
    take_exact_target_state_nonce(&NEXT_EXACT_TARGET_STATE_NONCE)
}

fn take_exact_target_state_nonce(
    source: &AtomicU64,
) -> Result<u64, GeneratedAffineResidualGroupExactTargetError> {
    source
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |nonce| {
            nonce.checked_add(1)
        })
        .map_err(|_| GeneratedAffineResidualGroupExactTargetError::StateIdentityExhaustion)
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactTargetError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactTargetError::AllocationFailure { resource }
    })?;
    Ok(values)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactTargetError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupExactTargetError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupExactTargetError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualGroupExactTargetError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

#[cfg(test)]
mod tests {
    use super::super::database::{
        GeneratedAffineResidualGroupExactDatabase, GeneratedAffineResidualGroupExactDatabaseLimits,
    };
    use super::super::physical_key::{
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
    };
    use super::super::physical_row::{
        GeneratedAffineResidualGroupExactPhysicalRow,
        GeneratedAffineResidualGroupExactPhysicalRowCompiler,
        GeneratedAffineResidualGroupExactPhysicalRowLimits,
    };
    use super::super::plan::GeneratedAffineResidualGroupSolvePlanLimits;
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
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
    };
    use crate::parametric_sector_formula_affine_terminal::{
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
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
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

    fn direct_uncovered_plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                SectorMask::try_from_bit_string("111").unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        assert!(source.attempts().is_empty());
        assert!(!source.row_span().rows().is_empty());
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::Uncovered,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let path = Arc::new(cursor.next_path().unwrap().unwrap());
        assert!(cursor.next_path().unwrap().is_none());
        let terminal = Arc::new(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                path,
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap(),
        );
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new_direct_formula_singleton(
                &family,
                &context,
                terminal,
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
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new_direct_formula_singleton(
                &family,
                &context,
                authority,
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    fn ready_plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
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
        assert!(!group.case_ordinals().is_empty());
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
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                inventory,
                authority,
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    fn equality_refinement_plan_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualGroupSolvePlan>,
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
            SectorMask::try_from_bit_string("001").unwrap(),
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
        let prior_inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let effective = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                prior_inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        let prior_queue = Arc::new(
            GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
                &family,
                &context,
                effective,
                GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::prior_effective(prior_queue),
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
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new(
                &family,
                &context,
                inventory,
                authority,
                frame,
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        (family, context, plan)
    }

    fn exact_database_for_plan(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
        database_epoch: usize,
    ) -> GeneratedAffineResidualGroupExactDatabase {
        GeneratedAffineResidualGroupExactDatabase::try_new(
            family,
            context,
            Arc::clone(plan),
            Arc::clone(plan.physical_frame()),
            database_epoch,
            GeneratedAffineResidualGroupExactDatabaseLimits::default(),
        )
        .unwrap()
    }

    fn production_exact_physical_row(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        plan: &Arc<GeneratedAffineResidualGroupSolvePlan>,
    ) -> Arc<GeneratedAffineResidualGroupExactPhysicalRow> {
        let frame = plan.physical_frame();
        for &case_ordinal in frame.case_ordinals() {
            let authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    family,
                    context,
                    Arc::clone(plan.inventory().unwrap()),
                    case_ordinal,
                    GeneratedAffineResidualCaseAuthorityLimits::default(),
                )
                .unwrap(),
            );
            let premises = match compile_generated_affine_residual_case_premises(
                family,
                context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    continue;
                }
            };
            let ordering = Arc::new(
                GeneratedAffineParametricOrderingCertificate::try_new(
                    family,
                    context,
                    Arc::clone(&authority),
                    GeneratedAffineParametricOrderingLimits::default(),
                )
                .unwrap(),
            );
            let schedule = Arc::new(
                GeneratedAffinePreparePointScheduleCertificate::compile(
                    family,
                    context,
                    Arc::clone(&ordering),
                    &authority,
                    0,
                    GeneratedAffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
                family,
                context,
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
            return Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile_from_reelimination_for_test(
                    family,
                    context,
                    certificate,
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            );
        }
        panic!("the generic affine-group fixture produced no authenticated physical row")
    }

    #[test]
    fn direct_catalog_has_exact_zero_inventory_profile_and_tight_positive_limits() {
        let (family, context, plan) =
            direct_uncovered_plan_fixture("exact-target-direct-resource-profile");
        let baseline = plan
            .compile_exact_target_catalog(
                &family,
                &context,
                GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            )
            .unwrap();
        let stats = baseline.stats();
        assert_eq!(
            baseline.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V2_SCHEMA
        );
        assert_eq!(
            baseline.source_kind(),
            GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
        );
        assert_eq!(stats.plan_replays, 1);
        assert_eq!(stats.same_group_target_collections, 0);
        assert_eq!(stats.targets, 1);
        assert_eq!(stats.locator_comparisons, DIRECT_TARGET_LOCATOR_COMPARISONS);
        assert_eq!(stats.target_handle_resolutions, 0);
        assert_eq!(stats.target_case_authentications, 0);
        assert_eq!(stats.target_authority_constructions, 0);
        assert_eq!(stats.premises_compilations, 1);
        assert_eq!(stats.ready_targets, 1);
        assert_eq!(stats.equality_refinement_targets, 0);
        assert_eq!(stats.retained_plan_references, 2);
        assert_eq!(stats.retained_authority_references, 2);
        assert!(stats.owner_retained_byte_envelope > catalog_base_retained_bytes(1).unwrap());
        assert!(stats.peak_staging_byte_envelope >= stats.owner_retained_byte_envelope);
        assert!(baseline.target_uses_exact_plan_authority_allocation_for_test(0));

        let mut exact = GeneratedAffineResidualGroupExactTargetCatalogLimits::default();
        exact.max_plan_replays = 1;
        exact.max_same_group_target_collections = 0;
        exact.max_targets = 1;
        exact.max_locator_comparisons = DIRECT_TARGET_LOCATOR_COMPARISONS;
        exact.max_target_handle_resolutions = 0;
        exact.max_target_case_authentications = 0;
        exact.max_target_authority_constructions = 0;
        exact.max_premises_compilations = 1;
        exact.max_ready_targets = 1;
        exact.max_equality_refinement_targets = 0;
        exact.max_retained_plan_references = 2;
        exact.max_retained_authority_references = 2;
        exact.max_owner_retained_byte_envelope = stats.owner_retained_byte_envelope;
        exact.max_peak_staging_byte_envelope = stats.peak_staging_byte_envelope;
        let exact_catalog = plan
            .compile_exact_target_catalog(&family, &context, exact)
            .unwrap();
        assert_eq!(exact_catalog.stats(), stats);
        exact_catalog.replay(&family, &context, &plan).unwrap();

        let assert_resource_limit =
            |limits: GeneratedAffineResidualGroupExactTargetCatalogLimits,
             resource: &'static str,
             requested: usize,
             limit: usize| {
                assert!(matches!(
                    plan.compile_exact_target_catalog(&family, &context, limits),
                    Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                        resource: actual_resource,
                        requested: actual_requested,
                        limit: actual_limit,
                    }) if actual_resource == resource
                        && actual_requested == requested
                        && actual_limit == limit
                ));
            };
        let mut below = exact;
        below.max_plan_replays = 0;
        assert_resource_limit(below, "exact target plan replays", 1, 0);
        let mut below = exact;
        below.max_targets = 0;
        assert_resource_limit(below, "exact targets", 1, 0);
        let mut below = exact;
        below.max_locator_comparisons = DIRECT_TARGET_LOCATOR_COMPARISONS - 1;
        assert_resource_limit(
            below,
            "exact target locator comparisons",
            DIRECT_TARGET_LOCATOR_COMPARISONS,
            DIRECT_TARGET_LOCATOR_COMPARISONS - 1,
        );
        let mut below = exact;
        below.max_premises_compilations = 0;
        assert_resource_limit(below, "exact target premise compilations", 1, 0);
        let mut below = exact;
        below.max_ready_targets = 0;
        assert_resource_limit(below, "exact ready targets", 1, 0);
        let mut below = exact;
        below.max_retained_plan_references = 1;
        assert_resource_limit(below, "exact target retained plan references", 2, 1);
        let mut below = exact;
        below.max_retained_authority_references = 1;
        assert_resource_limit(below, "exact target retained authority references", 2, 1);
        let base_retained = catalog_base_retained_bytes(1).unwrap();
        let mut below = exact;
        below.max_owner_retained_byte_envelope = base_retained - 1;
        assert_resource_limit(
            below,
            "exact target owner retained byte envelope",
            base_retained,
            base_retained - 1,
        );
        let mut below = exact;
        below.max_peak_staging_byte_envelope = base_retained - 1;
        assert_resource_limit(
            below,
            "exact target peak staging byte envelope",
            base_retained,
            base_retained - 1,
        );
        let mut below = exact;
        below.max_owner_retained_byte_envelope = stats.owner_retained_byte_envelope - 1;
        assert_resource_limit(
            below,
            "exact target owner retained byte envelope",
            stats.owner_retained_byte_envelope,
            stats.owner_retained_byte_envelope - 1,
        );
        let mut below = exact;
        below.max_peak_staging_byte_envelope = stats.peak_staging_byte_envelope - 1;
        assert_resource_limit(
            below,
            "exact target peak staging byte envelope",
            stats.peak_staging_byte_envelope,
            stats.peak_staging_byte_envelope - 1,
        );
    }

    #[test]
    fn catalog_preserves_plan_order_and_rejects_value_equal_foreign_plan() {
        let (family, context, plan) = ready_plan_fixture("exact-target-catalog-order-private");
        let catalog = Arc::new(
            plan.compile_exact_target_catalog(
                &family,
                &context,
                GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            catalog.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V1_SCHEMA
        );
        assert_eq!(catalog.group_ordinal(), plan.group_ordinal());
        assert_eq!(catalog.len(), plan.targets().len());
        assert!(!catalog.is_empty());
        assert_eq!(catalog.stats().targets(), plan.targets().len());
        assert_eq!(
            catalog.stats().ready_targets() + catalog.stats().equality_refinement_targets(),
            plan.targets().len()
        );
        for (solve_ordinal, (target, locator)) in
            catalog.targets.iter().zip(plan.targets()).enumerate()
        {
            assert_eq!(locator.solve_ordinal(), solve_ordinal);
            assert_eq!(target.locator(), *locator);
            assert!(Arc::ptr_eq(target.plan(), &plan));
            assert_eq!(target.authority().case_ordinal(), locator.case_ordinal());
            assert_eq!(target.authority().group_ordinal(), plan.group_ordinal());
            match target {
                GeneratedAffineResidualGroupExactTargetOutcome::Ready(target) => {
                    assert_eq!(target.domain.case_ordinal(), locator.case_ordinal());
                    assert!(target.domain.same_authority_allocation(&target.authority));
                }
                GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                    target,
                ) => {
                    assert_eq!(target.refinement.case_ordinal(), locator.case_ordinal());
                    assert!(
                        target
                            .refinement
                            .same_authority_allocation(&target.authority)
                    );
                }
            }
        }
        catalog.replay(&family, &context, &plan).unwrap();

        let foreign = Arc::new((*plan).clone());
        assert!(!Arc::ptr_eq(&plan, &foreign));
        assert_eq!(plan.targets(), foreign.targets());
        assert_eq!(plan.stable_manifest(), foreign.stable_manifest());
        assert_eq!(
            catalog.replay(&family, &context, &foreign),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation)
        );
    }

    #[test]
    fn inert_state_is_all_unresolved_and_exposes_no_mutable_bitmap() {
        let (family, context, plan) = ready_plan_fixture("exact-target-state-private");
        let catalog = Arc::new(
            plan.compile_exact_target_catalog(
                &family,
                &context,
                GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            )
            .unwrap(),
        );
        let database = exact_database_for_plan(&family, &context, &plan, 17);
        let state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        database
            .authenticate_target_state_binding(state.binding())
            .unwrap();
        assert_eq!(
            state.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_STATE_V1_SCHEMA
        );
        assert_eq!(state.group_ordinal(), plan.group_ordinal());
        assert_eq!(state.database_epoch(), 17);
        assert_eq!(state.state_version(), 0);
        assert_eq!(state.stats().dispositions(), catalog.len());
        assert_eq!(state.stats().unresolved(), catalog.len());
        assert_eq!(state.stats().consumed(), 0);
        let before = state.stats();
        let sibling = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        database
            .authenticate_target_state_binding(sibling.binding())
            .unwrap();
        assert!(!state.same_allocation(&sibling));
        assert_ne!(state.allocation_nonce, sibling.allocation_nonce);

        let view = state.authenticated_view(&family, &context).unwrap();
        database
            .authenticate_target_state_binding(view.binding())
            .unwrap();
        assert!(view.authenticates_state_allocation(&state));
        assert!(!view.authenticates_state_allocation(&sibling));
        assert_eq!(
            view.iter().collect::<Vec<_>>(),
            (0..catalog.len()).collect::<Vec<_>>()
        );
        for solve_ordinal in view.iter() {
            assert_eq!(view.is_unresolved(solve_ordinal), Ok(true));
            let source_strong_count = Arc::strong_count(&state);
            let retained = view.retain_target(solve_ordinal).unwrap();
            assert_eq!(Arc::strong_count(&state), source_strong_count + 1);
            assert_eq!(retained.solve_ordinal(), solve_ordinal);
            assert_eq!(retained.locator().solve_ordinal(), solve_ordinal);
            match (
                view.authenticated_target(solve_ordinal).unwrap(),
                retained,
            ) {
                (
                    GeneratedAffineResidualGroupAuthenticatedExactTargetView::Ready(target),
                    GeneratedAffineResidualGroupRetainedExactTarget::Ready(retained),
                ) => {
                    assert_eq!(target.locator().solve_ordinal(), solve_ordinal);
                    assert_eq!(target.case_ordinal(), target.domain().case_ordinal());
                    assert_eq!(target.group_ordinal(), plan.group_ordinal());
                    assert_eq!(target.premises(), target.domain().premises());
                    assert!(retained.authenticates_source_state(&state));
                    assert_eq!(*retained.locator(), target.locator());
                    assert!(std::ptr::eq(retained.domain(), target.domain()));
                    assert!(std::ptr::eq(
                        retained.premises().as_ptr(),
                        target.premises().as_ptr()
                    ));
                }
                (
                    GeneratedAffineResidualGroupAuthenticatedExactTargetView::RequiresAffineEqualityRefinement(target),
                    GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(retained),
                ) => {
                    assert_eq!(target.locator().solve_ordinal(), solve_ordinal);
                    assert_eq!(target.case_ordinal(), target.refinement().case_ordinal());
                    assert_eq!(target.group_ordinal(), plan.group_ordinal());
                    assert!(retained.authenticates_source_state(&state));
                    assert_eq!(*retained.locator(), target.locator());
                    assert!(std::ptr::eq(retained.refinement(), target.refinement()));
                }
                _ => panic!("retained exact target changed its typed outcome"),
            }
            assert_eq!(Arc::strong_count(&state), source_strong_count);
        }
        assert_eq!(
            view.is_unresolved(catalog.len()),
            Err(GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange)
        );
        assert!(matches!(
            view.retain_target(catalog.len()),
            Err(GeneratedAffineResidualGroupExactTargetError::TargetOutOfRange)
        ));
        drop(view);
        assert_eq!(state.state_version(), 0);
        assert_eq!(state.stats(), before);

        let foreign = Arc::new((*plan).clone());
        assert_eq!(
            state.replay(&family, &context, &foreign, plan.group_ordinal(), 17, 0,),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation)
        );

        let foreign_plan_database = exact_database_for_plan(&family, &context, &foreign, 17);
        assert!(matches!(
            GeneratedAffineResidualGroupExactTargetState::try_new(
                &family,
                &context,
                Arc::clone(&catalog),
                foreign_plan_database
                    .initial_target_state_binding_for_test()
                    .unwrap(),
                GeneratedAffineResidualGroupExactTargetStateLimits::default(),
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongPlanAllocation)
        ));

        let foreign_database = exact_database_for_plan(&family, &context, &plan, 17);
        let foreign_database_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            foreign_database
                .initial_target_state_binding_for_test()
                .unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        assert!(
            database
                .authenticate_target_state_binding(foreign_database_state.binding())
                .is_err()
        );
        assert!(
            foreign_database
                .authenticate_target_state_binding(state.binding())
                .is_err()
        );
    }

    #[test]
    fn successor_is_immutable_database_bound_and_consumes_one_ready_handle() {
        let (family, context, plan) = ready_plan_fixture("exact-target-successor-private");
        let catalog = Arc::new(
            plan.compile_exact_target_catalog(
                &family,
                &context,
                GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            )
            .unwrap(),
        );
        let ready_ordinal = catalog
            .targets
            .iter()
            .position(|target| {
                matches!(
                    target,
                    GeneratedAffineResidualGroupExactTargetOutcome::Ready(_)
                )
            })
            .expect("the Ready fixture must retain a consumable exact target");
        let mut database = exact_database_for_plan(&family, &context, &plan, 31);
        let mut initial_group_limited =
            GeneratedAffineResidualGroupExactTargetStateLimits::default();
        initial_group_limited.max_group_comparisons = 1;
        assert!(matches!(
            GeneratedAffineResidualGroupExactTargetState::try_new(
                &family,
                &context,
                Arc::clone(&catalog),
                database.initial_target_state_binding_for_test().unwrap(),
                initial_group_limited,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target group comparisons",
                    requested: 2,
                    limit: 1,
                }
            )
        ));
        let mut initial_state_envelope_limited =
            GeneratedAffineResidualGroupExactTargetStateLimits::default();
        initial_state_envelope_limited.max_state_retained_byte_envelope = 0;
        assert!(matches!(
            GeneratedAffineResidualGroupExactTargetState::try_new(
                &family,
                &context,
                Arc::clone(&catalog),
                database.initial_target_state_binding_for_test().unwrap(),
                initial_state_envelope_limited,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource: "exact target state retained byte envelope",
                requested,
                limit: 0,
            }) if requested > 0
        ));
        let mut initial_combined_envelope_limited =
            GeneratedAffineResidualGroupExactTargetStateLimits::default();
        initial_combined_envelope_limited.max_combined_retained_byte_envelope = 0;
        assert!(matches!(
            GeneratedAffineResidualGroupExactTargetState::try_new(
                &family,
                &context,
                Arc::clone(&catalog),
                database.initial_target_state_binding_for_test().unwrap(),
                initial_combined_envelope_limited,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource: "exact target combined retained byte envelope",
                requested,
                limit: 0,
            }) if requested > 0
        ));
        let state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        let sibling = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        let mut zero_publication_replay_state =
            GeneratedAffineResidualGroupExactTargetState::try_new(
                &family,
                &context,
                Arc::clone(&catalog),
                database.initial_target_state_binding_for_test().unwrap(),
                GeneratedAffineResidualGroupExactTargetStateLimits::default(),
            )
            .unwrap();
        Arc::get_mut(&mut zero_publication_replay_state)
            .unwrap()
            .limits
            .max_catalog_replays = 0;
        let mut copy_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        copy_limited.max_disposition_copies = catalog.len() - 1;
        let copy_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            copy_limited,
        )
        .unwrap();
        let mut peak_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        peak_limited.max_successor_peak_retained_byte_envelope = 0;
        let peak_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            peak_limited,
        )
        .unwrap();
        let mut allocation_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        allocation_limited.max_database_allocation_comparisons = 1;
        let allocation_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            allocation_limited,
        )
        .unwrap();
        let mut version_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        version_limited.max_state_version_comparisons = 1;
        let version_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            version_limited,
        )
        .unwrap();
        let mut group_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        group_limited.max_group_comparisons = 2;
        let group_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            group_limited,
        )
        .unwrap();
        let mut transition_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        transition_limited.max_predecessor_transition_comparisons = 0;
        let transition_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            transition_limited,
        )
        .unwrap();
        let mut epoch_limited = GeneratedAffineResidualGroupExactTargetStateLimits::default();
        epoch_limited.max_database_epoch_comparisons = 0;
        let epoch_limited_state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            epoch_limited,
        )
        .unwrap();
        let source_stats = state.stats();
        let source_nonce = state.allocation_nonce;

        let physical_row = production_exact_physical_row(&family, &context, &plan);
        let staged = database
            .stage_replayed_row_for_test(
                &family,
                &context,
                &plan,
                plan.physical_frame(),
                database.database_epoch(),
                &physical_row,
            )
            .unwrap();
        let competing_staged = database
            .stage_replayed_row_for_test(
                &family,
                &context,
                &plan,
                plan.physical_frame(),
                database.database_epoch(),
                &physical_row,
            )
            .unwrap();
        let abandoned_branch = state
            .prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&competing_staged)
                    .unwrap(),
                None,
            )
            .unwrap();
        drop(competing_staged);

        let foreign_database = exact_database_for_plan(&family, &context, &plan, 31);
        let foreign_staged = foreign_database
            .stage_replayed_row_for_test(
                &family,
                &context,
                &plan,
                plan.physical_frame(),
                foreign_database.database_epoch(),
                &physical_row,
            )
            .unwrap();
        assert!(matches!(
            state.prepare_successor(
                &family,
                &context,
                foreign_database
                    .successor_target_state_binding_for_test(&foreign_staged)
                    .unwrap(),
                None,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongDatabaseAllocation)
        ));

        let sibling_handle = match sibling
            .authenticated_view(&family, &context)
            .unwrap()
            .retain_target(ready_ordinal)
            .unwrap()
        {
            GeneratedAffineResidualGroupRetainedExactTarget::Ready(target) => target,
            GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(
                _,
            ) => panic!("the selected Ready target changed typed outcome"),
        };
        assert!(matches!(
            state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                Some(sibling_handle),
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongSourceStateAllocation)
        ));
        let zero_replay_handle = GeneratedAffineResidualGroupRetainedReadyExactTarget {
            state: Arc::clone(&zero_publication_replay_state),
            solve_ordinal: ready_ordinal,
        };
        let zero_replay_successor = zero_publication_replay_state
            .prepare_publication_successor(
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                &zero_replay_handle,
            )
            .unwrap();
        assert_eq!(zero_replay_successor.stats().catalog_replays, 0);
        assert_eq!(zero_replay_successor.stats().consumed(), 1);
        assert!(matches!(
            copy_limited_state.prepare_successor(
                &family,
                &context,
                database.successor_target_state_binding_for_test(&staged).unwrap(),
                None,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource: "exact target disposition copies",
                requested,
                limit,
            }) if requested == catalog.len() && limit == catalog.len() - 1
        ));
        assert_eq!(copy_limited_state.state_version(), 0);
        assert_eq!(copy_limited_state.stats().consumed(), 0);
        assert!(matches!(
            peak_limited_state.prepare_successor(
                &family,
                &context,
                database.successor_target_state_binding_for_test(&staged).unwrap(),
                None,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource: "exact target successor peak retained byte envelope",
                requested,
                limit: 0,
            }) if requested > 0
        ));
        assert_eq!(peak_limited_state.state_version(), 0);
        assert_eq!(peak_limited_state.stats().consumed(), 0);
        assert!(matches!(
            allocation_limited_state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target database allocation comparisons",
                    requested: 2,
                    limit: 1,
                }
            )
        ));
        assert_eq!(allocation_limited_state.state_version(), 0);
        assert_eq!(allocation_limited_state.stats().consumed(), 0);
        assert!(matches!(
            version_limited_state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target state version comparisons",
                    requested: 2,
                    limit: 1,
                }
            )
        ));
        assert_eq!(version_limited_state.state_version(), 0);
        assert_eq!(version_limited_state.stats().consumed(), 0);
        assert!(matches!(
            group_limited_state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target group comparisons",
                    requested: 3,
                    limit: 2,
                }
            )
        ));
        assert_eq!(group_limited_state.stats().group_comparisons(), 2);
        assert!(matches!(
            transition_limited_state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target predecessor transition comparisons",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
        assert_eq!(
            transition_limited_state
                .stats()
                .predecessor_transition_comparisons(),
            0
        );
        assert!(matches!(
            epoch_limited_state.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            ),
            Err(
                GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                    resource: "exact target database epoch comparisons",
                    requested: 1,
                    limit: 0,
                }
            )
        ));
        assert_eq!(epoch_limited_state.stats().database_epoch_comparisons, 0);

        let unchanged = state
            .prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                None,
            )
            .unwrap();
        let ready_handle = match state
            .authenticated_view(&family, &context)
            .unwrap()
            .retain_target(ready_ordinal)
            .unwrap()
        {
            GeneratedAffineResidualGroupRetainedExactTarget::Ready(target) => target,
            GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(
                _,
            ) => panic!("the selected Ready target changed typed outcome"),
        };
        assert!(ready_handle.authenticates_source_state(&state));
        let consumed = state
            .prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&staged)
                    .unwrap(),
                Some(ready_handle),
            )
            .unwrap();

        assert!(!state.same_allocation(&unchanged));
        assert!(!state.same_allocation(&consumed));
        assert!(!unchanged.same_allocation(&consumed));
        assert_eq!(state.state_version(), 0);
        assert_eq!(unchanged.state_version(), 1);
        assert_eq!(consumed.state_version(), 1);
        assert_eq!(state.stats(), source_stats);
        assert_eq!(state.allocation_nonce, source_nonce);
        assert_eq!(state.stats().consumed(), 0);
        assert_eq!(unchanged.stats().consumed(), 0);
        assert_eq!(consumed.stats().consumed(), 1);
        assert_eq!(unchanged.stats().disposition_copies(), catalog.len());
        assert_eq!(consumed.stats().disposition_copies(), catalog.len());
        assert_eq!(unchanged.stats().target_consumptions(), 0);
        assert_eq!(consumed.stats().target_consumptions(), 1);
        assert_eq!(state.stats().group_comparisons(), 2);
        assert_eq!(unchanged.stats().group_comparisons(), 3);
        assert_eq!(consumed.stats().group_comparisons(), 3);
        assert_eq!(state.stats().predecessor_transition_comparisons(), 0);
        assert_eq!(unchanged.stats().predecessor_transition_comparisons(), 1);
        assert_eq!(consumed.stats().predecessor_transition_comparisons(), 1);
        assert_eq!(state.stats().database_epoch_comparisons, 0);
        assert_eq!(unchanged.stats().database_epoch_comparisons, 1);
        assert_eq!(consumed.stats().database_epoch_comparisons, 1);
        assert!(
            consumed.stats().successor_peak_retained_byte_envelope()
                > consumed.stats().combined_retained_byte_envelope()
        );
        assert!(Arc::ptr_eq(&state.catalog, &unchanged.catalog));
        assert!(Arc::ptr_eq(&state.catalog, &consumed.catalog));

        let source_view = state.authenticated_view(&family, &context).unwrap();
        let unchanged_view = unchanged.authenticated_view(&family, &context).unwrap();
        let consumed_view = consumed.authenticated_view(&family, &context).unwrap();
        for solve_ordinal in source_view.iter() {
            assert_eq!(source_view.is_unresolved(solve_ordinal), Ok(true));
            assert_eq!(unchanged_view.is_unresolved(solve_ordinal), Ok(true));
            assert_eq!(
                consumed_view.is_unresolved(solve_ordinal),
                Ok(solve_ordinal != ready_ordinal)
            );
        }
        assert_eq!(
            consumed_view.authenticated_target(ready_ordinal).err(),
            Some(GeneratedAffineResidualGroupExactTargetError::TargetConsumed)
        );
        assert!(matches!(
            consumed_view.retain_target(ready_ordinal),
            Err(GeneratedAffineResidualGroupExactTargetError::TargetConsumed)
        ));

        assert!(
            database
                .authenticate_target_state_binding(unchanged.binding())
                .is_err()
        );
        assert!(
            database
                .authenticate_target_state_binding(consumed.binding())
                .is_err()
        );
        database.commit_staged_row_for_test(staged).unwrap();
        database
            .authenticate_target_state_binding(unchanged.binding())
            .unwrap();
        database
            .authenticate_target_state_binding(consumed.binding())
            .unwrap();
        assert!(
            database
                .authenticate_target_state_binding(abandoned_branch.binding())
                .is_err()
        );
        assert!(
            database
                .authenticate_target_state_binding(state.binding())
                .is_err()
        );

        let live_continuation = database
            .stage_replayed_row_for_test(
                &family,
                &context,
                &plan,
                plan.physical_frame(),
                database.database_epoch(),
                &physical_row,
            )
            .unwrap();
        assert!(matches!(
            abandoned_branch.prepare_successor(
                &family,
                &context,
                database
                    .successor_target_state_binding_for_test(&live_continuation)
                    .unwrap(),
                None,
            ),
            Err(GeneratedAffineResidualGroupExactTargetError::WrongPredecessorTransition)
        ));
        assert_eq!(abandoned_branch.state_version(), 1);
        assert_eq!(abandoned_branch.stats().consumed(), 0);
    }

    #[test]
    fn equality_predicates_remain_typed_non_ready_targets() {
        let (family, context, plan) =
            equality_refinement_plan_fixture("exact-target-refinement-private");
        let catalog = Arc::new(
            plan.compile_exact_target_catalog(
                &family,
                &context,
                GeneratedAffineResidualGroupExactTargetCatalogLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(catalog.stats().ready_targets(), 0);
        assert_eq!(catalog.stats().equality_refinement_targets(), catalog.len());
        for (outcome, locator) in catalog.targets.iter().zip(plan.targets()) {
            let GeneratedAffineResidualGroupExactTargetOutcome::RequiresAffineEqualityRefinement(
                target,
            ) = outcome
            else {
                panic!("an equality-bearing target must never become Ready or NoTarget")
            };
            assert_eq!(target.locator, *locator);
            assert_eq!(target.refinement.case_ordinal(), locator.case_ordinal());
            assert!(!target.refinement.equality_predicate_ordinals().is_empty());
        }
        catalog.replay(&family, &context, &plan).unwrap();

        let database = exact_database_for_plan(&family, &context, &plan, 23);
        let state = GeneratedAffineResidualGroupExactTargetState::try_new(
            &family,
            &context,
            Arc::clone(&catalog),
            database.initial_target_state_binding_for_test().unwrap(),
            GeneratedAffineResidualGroupExactTargetStateLimits::default(),
        )
        .unwrap();
        let view = state.authenticated_view(&family, &context).unwrap();
        for solve_ordinal in view.iter() {
            let retained = view.retain_target(solve_ordinal).unwrap();
            let GeneratedAffineResidualGroupRetainedExactTarget::RequiresAffineEqualityRefinement(
                retained,
            ) = retained
            else {
                panic!("an equality-bearing target acquired a Ready retained handle")
            };
            assert!(retained.authenticates_source_state(&state));
            assert_eq!(retained.solve_ordinal(), solve_ordinal);
            assert_eq!(retained.locator().solve_ordinal(), solve_ordinal);
            assert!(
                !retained
                    .refinement()
                    .equality_predicate_ordinals()
                    .is_empty()
            );
        }
    }

    #[test]
    fn target_count_limit_rejects_before_catalog_allocation() {
        let (family, context, plan) = ready_plan_fixture("exact-target-count-limit-private");
        assert!(!plan.targets().is_empty());
        let mut limits = GeneratedAffineResidualGroupExactTargetCatalogLimits::default();
        limits.max_targets = plan.targets().len() - 1;
        assert!(matches!(
            plan.compile_exact_target_catalog(&family, &context, limits),
            Err(GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                resource: "exact targets",
                requested,
                limit,
            }) if requested == plan.targets().len() && limit == plan.targets().len() - 1
        ));
    }

    #[test]
    fn state_identity_source_is_unique_and_never_wraps() {
        let source = AtomicU64::new(41);
        assert_eq!(take_exact_target_state_nonce(&source), Ok(41));
        assert_eq!(take_exact_target_state_nonce(&source), Ok(42));

        let exhausted = AtomicU64::new(u64::MAX - 1);
        assert_eq!(take_exact_target_state_nonce(&exhausted), Ok(u64::MAX - 1));
        assert_eq!(exhausted.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(
            take_exact_target_state_nonce(&exhausted),
            Err(GeneratedAffineResidualGroupExactTargetError::StateIdentityExhaustion)
        );
        assert_eq!(exhausted.load(Ordering::Relaxed), u64::MAX);
    }
}
