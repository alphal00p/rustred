//! Ownership-safe schedule foundation for exact Ready condition compilation.
//!
//! This layer consumes the current-lineage `ReadyForConditions` capability
//! only after it has reauthenticated the complete Ready transcript, bound an
//! authority-neutral affine target transform, and retained a deterministic
//! ordinal schedule.  It deliberately does not map a coefficient, construct a
//! Boolean condition, partition a domain, consume a target, or publish a rule.
//! Its limits bound each newly retained component and the resulting logical
//! payload.  The compact child compiler performs its own preflight; because
//! that child is allocated before its exact logical census is available, this
//! V1 foundation is transactionally recoverable but is not an owner-wide
//! pre-allocation ledger.  The consuming condition/materialization phase must
//! perform that complete-row admission before composing any source.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::generated_affine_residual_group_exact_session::{
    GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionError,
};
use crate::generated_affine_residual_group_ready_publication::{
    GeneratedAffineResidualGroupReadyForConditions,
    GeneratedAffineResidualGroupReadyPublicationAnalysisError,
};
use crate::parametric_coefficient::{
    ResidualAffineCompactCompositionPlan, ResidualAffineCompactCompositionPlanLimits,
    ResidualAffineCompactMapView, ResidualUnitAffineCompositionError,
};
use crate::{IntegralFamily, ParametricCoefficientContext};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_CONDITION_PLAN_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-exact-condition-plan-v1";

#[cfg(test)]
std::thread_local! {
    static CONDITION_PLAN_BOUNDARY_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn inject_condition_plan_boundary_panic_for_test() {
    CONDITION_PLAN_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_condition_plan_boundary_panic_for_test() {
    CONDITION_PLAN_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected exact condition-plan boundary panic");
        }
    });
}

#[cfg(not(test))]
fn maybe_inject_condition_plan_boundary_panic_for_test() {}

/// Per-attempt incremental limits.  The already-live Ready graph is excluded;
/// every vector and compact-transform allocation newly owned by this plan is
/// included in its final census. Replay applies the same envelope to the newly
/// rebuilt transcript and excludes the caller-owned plan.  This V1 envelope is
/// not the future condition phase's owner-wide pre-allocation ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactConditionPlanLimits {
    pub(crate) target_transform: ResidualAffineCompactCompositionPlanLimits,
    pub(crate) max_target_geometry_entries_inspected: usize,
    pub(crate) max_premise_sources: usize,
    pub(crate) max_row_guard_sources: usize,
    pub(crate) max_coefficient_sources: usize,
    pub(crate) max_source_schedule_entries: usize,
    pub(crate) max_hazard_locators: usize,
    pub(crate) max_source_schedule_retained_bytes: usize,
    pub(crate) max_hazard_schedule_retained_bytes: usize,
    pub(crate) max_retained_owned_logical_bytes: usize,
    pub(crate) max_compilation_owned_logical_peak_upper_bound: usize,
}

impl Default for GeneratedAffineResidualGroupExactConditionPlanLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const GIB: usize = 1024 * 1024 * 1024;
        Self {
            target_transform: ResidualAffineCompactCompositionPlanLimits::default(),
            max_target_geometry_entries_inspected: LARGE,
            max_premise_sources: 16_000_000,
            max_row_guard_sources: 16_000_000,
            max_coefficient_sources: 16_000_000,
            max_source_schedule_entries: 48_000_000,
            max_hazard_locators: LARGE,
            max_source_schedule_retained_bytes: 4 * GIB,
            max_hazard_schedule_retained_bytes: 4 * GIB,
            max_retained_owned_logical_bytes: 16 * GIB,
            max_compilation_owned_logical_peak_upper_bound: 32 * GIB,
        }
    }
}

/// Deterministic, allocation-independent census for one retained plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactConditionPlanStats {
    target_geometry_entries_inspected: usize,
    premise_sources: usize,
    row_guard_sources: usize,
    coefficient_sources: usize,
    source_schedule_entries: usize,
    hazard_locators: usize,
    identity_target_transform: bool,
    target_transform_retained_owned_logical_bytes: usize,
    target_transform_compilation_owned_logical_peak_upper_bound: usize,
    source_schedule_retained_bytes: usize,
    hazard_schedule_retained_bytes: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

macro_rules! condition_plan_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupExactConditionPlanStats {
    condition_plan_stats_getters!(
        target_geometry_entries_inspected,
        premise_sources,
        row_guard_sources,
        coefficient_sources,
        source_schedule_entries,
        hazard_locators,
        target_transform_retained_owned_logical_bytes,
        target_transform_compilation_owned_logical_peak_upper_bound,
        source_schedule_retained_bytes,
        hazard_schedule_retained_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    );

    pub(crate) const fn identity_target_transform(self) -> bool {
        self.identity_target_transform
    }
}

/// Exact source ordinal to be mapped by the next compilation phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactConditionSourceLocator {
    TargetPremise {
        premise_ordinal: usize,
    },
    RecenteredRowGuard {
        guard_ordinal: usize,
    },
    PivotCoefficient {
        term_ordinal: usize,
    },
    RhsCoefficient {
        rhs_ordinal: usize,
        term_ordinal: usize,
    },
}

/// Lazy locator into the exact hazard range already owned by Ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactConditionHazardLocator {
    hazard_ordinal: usize,
    rhs_ordinal: usize,
    term_ordinal: usize,
    coordinate: usize,
}

impl GeneratedAffineResidualGroupExactConditionHazardLocator {
    pub(crate) const fn hazard_ordinal(self) -> usize {
        self.hazard_ordinal
    }

    pub(crate) const fn rhs_ordinal(self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn term_ordinal(self) -> usize {
        self.term_ordinal
    }

    pub(crate) const fn coordinate(self) -> usize {
        self.coordinate
    }
}

enum GeneratedAffineResidualGroupExactConditionTargetTransform {
    Identity { ambient_arity: usize },
    Compact(ResidualAffineCompactCompositionPlan),
}

impl GeneratedAffineResidualGroupExactConditionTargetTransform {
    const fn is_identity(&self) -> bool {
        matches!(self, Self::Identity { .. })
    }

    const fn compact(&self) -> Option<&ResidualAffineCompactCompositionPlan> {
        match self {
            Self::Identity { .. } => None,
            Self::Compact(plan) => Some(plan),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactConditionTargetTransform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity { ambient_arity } => formatter
                .debug_struct("Identity")
                .field("ambient_arity", ambient_arity)
                .finish(),
            Self::Compact(plan) => formatter
                .debug_struct("Compact")
                .field("manifest", &plan.manifest())
                .finish(),
        }
    }
}

/// Non-Clone owner for the next exact WhenBad phase.
pub(crate) struct GeneratedAffineResidualGroupExactConditionPlan {
    schema: &'static str,
    ready: GeneratedAffineResidualGroupReadyForConditions,
    target_transform: GeneratedAffineResidualGroupExactConditionTargetTransform,
    source_schedule: Vec<GeneratedAffineResidualGroupExactConditionSourceLocator>,
    hazard_schedule: Vec<GeneratedAffineResidualGroupExactConditionHazardLocator>,
    limits: GeneratedAffineResidualGroupExactConditionPlanLimits,
    stats: GeneratedAffineResidualGroupExactConditionPlanStats,
}

impl GeneratedAffineResidualGroupExactConditionPlan {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupExactConditionPlanLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupExactConditionPlanStats {
        self.stats
    }

    pub(crate) const fn target_transform_is_identity(&self) -> bool {
        self.target_transform.is_identity()
    }

    pub(crate) const fn compact_target_transform(
        &self,
    ) -> Option<&ResidualAffineCompactCompositionPlan> {
        self.target_transform.compact()
    }

    pub(crate) fn source_schedule(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactConditionSourceLocator] {
        &self.source_schedule
    }

    pub(crate) fn hazard_schedule(
        &self,
    ) -> &[GeneratedAffineResidualGroupExactConditionHazardLocator] {
        &self.hazard_schedule
    }

    pub(crate) const fn ready(&self) -> &GeneratedAffineResidualGroupReadyForConditions {
        &self.ready
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) const fn publishes_rule(&self) -> bool {
        false
    }

    /// Rebuild and compare the complete plan transcript.  This reauthenticates
    /// the exact Ready/session allocation first, so a value-equal foreign
    /// target-state allocation cannot replay.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactConditionPlanError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_CONDITION_PLAN_V1_SCHEMA {
                return Err(GeneratedAffineResidualGroupExactConditionPlanError::SchemaMismatch);
            }
            self.replay_target_transform(family, context, session)?;
            let rebuilt =
                prepare_condition_plan(family, context, session, &self.ready, self.limits)?;
            if rebuilt.source_schedule != self.source_schedule
                || rebuilt.hazard_schedule != self.hazard_schedule
                || rebuilt.stats != self.stats
                || !target_transforms_match(&self.target_transform, &rebuilt.target_transform)
            {
                return Err(GeneratedAffineResidualGroupExactConditionPlanError::ReplayMismatch);
            }
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualGroupExactConditionPlanError::SymbolicaPanic)?
    }

    fn replay_target_transform(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupExactConditionPlanError> {
        let geometry = session.authenticated_ready_geometry(family, context, self.ready.ready())?;
        match &self.target_transform {
            GeneratedAffineResidualGroupExactConditionTargetTransform::Identity {
                ambient_arity,
            } => {
                if *ambient_arity != geometry.ambient_arity()
                    || !target_transform_is_identity(
                        geometry.ambient_arity(),
                        geometry.target_offset(),
                        geometry.free_positions(),
                        geometry.compact_affine_matrix(),
                    )?
                {
                    return Err(
                        GeneratedAffineResidualGroupExactConditionPlanError::ReplayMismatch,
                    );
                }
            }
            GeneratedAffineResidualGroupExactConditionTargetTransform::Compact(plan) => {
                let map = ResidualAffineCompactMapView::new(
                    context.fingerprint(),
                    geometry.ambient_arity(),
                    geometry.target_offset(),
                    geometry.free_positions(),
                    geometry.compact_affine_matrix(),
                );
                plan.replay(context, map)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactConditionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactConditionPlan")
            .field("schema", &self.schema)
            .field("target_transform", &self.target_transform)
            .field("source_schedule_entries", &self.source_schedule.len())
            .field("hazard_locators", &self.hazard_schedule.len())
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("publishes_rule", &false)
            .field("private_ready", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupExactConditionPlanError {
    Ready(GeneratedAffineResidualGroupReadyPublicationAnalysisError),
    Session(GeneratedAffineResidualGroupExactSessionError),
    TargetTransform(ResidualUnitAffineCompositionError),
    SchemaMismatch,
    ReplayMismatch,
    MalformedReady,
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
    SymbolicaPanic,
}

impl fmt::Display for GeneratedAffineResidualGroupExactConditionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ready(_) => "exact Ready transcript authentication failed",
            Self::Session(_) => "exact Ready/session geometry authentication failed",
            Self::TargetTransform(_) => "exact condition target-transform compilation failed",
            Self::SchemaMismatch => "exact condition-plan schema mismatch",
            Self::ReplayMismatch => "exact condition-plan replay mismatch",
            Self::MalformedReady => "exact condition source schedule is malformed",
            Self::ResourceLimit { .. } => "exact condition-plan resource limit exceeded",
            Self::ResourceCountOverflow { .. } => "exact condition-plan resource count overflow",
            Self::AllocationFailure { .. } => "exact condition-plan bounded allocation failed",
            Self::SymbolicaPanic => "Symbolica panicked inside the exact condition-plan boundary",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactConditionPlanError {}

impl From<GeneratedAffineResidualGroupReadyPublicationAnalysisError>
    for GeneratedAffineResidualGroupExactConditionPlanError
{
    fn from(error: GeneratedAffineResidualGroupReadyPublicationAnalysisError) -> Self {
        Self::Ready(error)
    }
}

impl From<GeneratedAffineResidualGroupExactSessionError>
    for GeneratedAffineResidualGroupExactConditionPlanError
{
    fn from(error: GeneratedAffineResidualGroupExactSessionError) -> Self {
        Self::Session(error)
    }
}

impl From<ResidualUnitAffineCompositionError>
    for GeneratedAffineResidualGroupExactConditionPlanError
{
    fn from(error: ResidualUnitAffineCompositionError) -> Self {
        Self::TargetTransform(error)
    }
}

/// Recoverable failure retaining the exact non-Clone ReadyForConditions owner.
pub(crate) struct GeneratedAffineResidualGroupExactConditionPlanFailure {
    error: GeneratedAffineResidualGroupExactConditionPlanError,
    ready: GeneratedAffineResidualGroupReadyForConditions,
}

impl GeneratedAffineResidualGroupExactConditionPlanFailure {
    pub(crate) const fn error(&self) -> &GeneratedAffineResidualGroupExactConditionPlanError {
        &self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedAffineResidualGroupExactConditionPlanError,
        GeneratedAffineResidualGroupReadyForConditions,
    ) {
        (self.error, self.ready)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactConditionPlanFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactConditionPlanFailure")
            .field("error", &self.error)
            .field("private_ready", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupExactConditionPlanFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for GeneratedAffineResidualGroupExactConditionPlanFailure {}

pub(crate) struct GeneratedAffineResidualGroupExactConditionPlanCompiler;

impl GeneratedAffineResidualGroupExactConditionPlanCompiler {
    /// Compile without mutating the session. Every ordinary error and caught
    /// panic returns the exact input owner, not a reconstructed equivalent.
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
        ready: GeneratedAffineResidualGroupReadyForConditions,
        limits: GeneratedAffineResidualGroupExactConditionPlanLimits,
    ) -> Result<
        GeneratedAffineResidualGroupExactConditionPlan,
        GeneratedAffineResidualGroupExactConditionPlanFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            prepare_condition_plan(family, context, session, &ready, limits)
        }));
        match prepared {
            Ok(Ok(prepared)) => Ok(GeneratedAffineResidualGroupExactConditionPlan {
                schema: GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_CONDITION_PLAN_V1_SCHEMA,
                ready,
                target_transform: prepared.target_transform,
                source_schedule: prepared.source_schedule,
                hazard_schedule: prepared.hazard_schedule,
                limits,
                stats: prepared.stats,
            }),
            Ok(Err(error)) => {
                Err(GeneratedAffineResidualGroupExactConditionPlanFailure { error, ready })
            }
            Err(_) => Err(GeneratedAffineResidualGroupExactConditionPlanFailure {
                error: GeneratedAffineResidualGroupExactConditionPlanError::SymbolicaPanic,
                ready,
            }),
        }
    }
}

struct PreparedConditionPlan {
    target_transform: GeneratedAffineResidualGroupExactConditionTargetTransform,
    source_schedule: Vec<GeneratedAffineResidualGroupExactConditionSourceLocator>,
    hazard_schedule: Vec<GeneratedAffineResidualGroupExactConditionHazardLocator>,
    stats: GeneratedAffineResidualGroupExactConditionPlanStats,
}

fn prepare_condition_plan(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    ready: &GeneratedAffineResidualGroupReadyForConditions,
    limits: GeneratedAffineResidualGroupExactConditionPlanLimits,
) -> Result<PreparedConditionPlan, GeneratedAffineResidualGroupExactConditionPlanError> {
    maybe_inject_condition_plan_boundary_panic_for_test();
    ready.replay(family, context, session)?;
    let geometry = session.authenticated_ready_geometry(family, context, ready.ready())?;
    let ambient_arity = geometry.ambient_arity();
    let matrix_entries = checked_mul(
        "exact condition target geometry entries",
        ambient_arity,
        geometry.free_positions().len(),
    )?;
    let geometry_entries = checked_add(
        "exact condition target geometry entries",
        checked_add(
            "exact condition target geometry entries",
            ambient_arity,
            geometry.free_positions().len(),
        )?,
        matrix_entries,
    )?;
    check_limit(
        "exact condition target geometry entries",
        geometry_entries,
        limits.max_target_geometry_entries_inspected,
    )?;

    let identity = target_transform_is_identity(
        ambient_arity,
        geometry.target_offset(),
        geometry.free_positions(),
        geometry.compact_affine_matrix(),
    )?;
    let premise_sources = ready.ready().target_premises().len();
    let row_guard_sources = ready.ready().row_guards().len();
    let coefficient_sources = ready.ready().terms().len();
    check_limit(
        "exact condition target-premise sources",
        premise_sources,
        limits.max_premise_sources,
    )?;
    check_limit(
        "exact condition row-guard sources",
        row_guard_sources,
        limits.max_row_guard_sources,
    )?;
    check_limit(
        "exact condition coefficient sources",
        coefficient_sources,
        limits.max_coefficient_sources,
    )?;
    let source_schedule_entries = checked_add(
        "exact condition source schedule entries",
        checked_add(
            "exact condition source schedule entries",
            premise_sources,
            row_guard_sources,
        )?,
        coefficient_sources,
    )?;
    check_limit(
        "exact condition source schedule entries",
        source_schedule_entries,
        limits.max_source_schedule_entries,
    )?;
    let hazard_locators = ready.hazards().len();
    check_limit(
        "exact condition hazard locators",
        hazard_locators,
        limits.max_hazard_locators,
    )?;

    validate_ready_ordinals(ready)?;

    let prospective_source_bytes = checked_mul(
        "exact condition source schedule retained bytes",
        source_schedule_entries,
        size_of::<GeneratedAffineResidualGroupExactConditionSourceLocator>(),
    )?;
    check_limit(
        "exact condition source schedule retained bytes",
        prospective_source_bytes,
        limits.max_source_schedule_retained_bytes,
    )?;
    let prospective_hazard_bytes = checked_mul(
        "exact condition hazard schedule retained bytes",
        hazard_locators,
        size_of::<GeneratedAffineResidualGroupExactConditionHazardLocator>(),
    )?;
    check_limit(
        "exact condition hazard schedule retained bytes",
        prospective_hazard_bytes,
        limits.max_hazard_schedule_retained_bytes,
    )?;

    // All cheap source/schedule admission and ordinal authentication precedes
    // compact Symbolica-plan allocation.  The child plan itself has a sealed
    // prospective preflight; its exact retained census becomes available only
    // after compilation and is combined with the still-unallocated schedules
    // immediately below.
    let target_transform = if identity {
        GeneratedAffineResidualGroupExactConditionTargetTransform::Identity { ambient_arity }
    } else {
        let map = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            ambient_arity,
            geometry.target_offset(),
            geometry.free_positions(),
            geometry.compact_affine_matrix(),
        );
        GeneratedAffineResidualGroupExactConditionTargetTransform::Compact(
            context
                .compile_residual_affine_compact_composition_plan(map, limits.target_transform)?,
        )
    };
    let (target_retained, target_peak) = target_transform_logical_memory(&target_transform)?;
    let outer_shell = condition_plan_outer_shell_bytes()?;
    let prospective_schedule_bytes = checked_add(
        "exact condition-plan retained owned logical bytes",
        prospective_source_bytes,
        prospective_hazard_bytes,
    )?;
    let prospective_retained = checked_add(
        "exact condition-plan retained owned logical bytes",
        outer_shell,
        checked_add(
            "exact condition-plan retained owned logical bytes",
            target_retained,
            prospective_schedule_bytes,
        )?,
    )?;
    check_limit(
        "exact condition-plan retained owned logical bytes",
        prospective_retained,
        limits.max_retained_owned_logical_bytes,
    )?;
    let prospective_peak = checked_add(
        "exact condition-plan compilation owned logical peak upper bound",
        outer_shell,
        target_peak,
    )?
    .max(prospective_retained);
    check_limit(
        "exact condition-plan compilation owned logical peak upper bound",
        prospective_peak,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    let mut source_schedule =
        try_vec_with_capacity("exact condition source schedule", source_schedule_entries)?;
    for premise_ordinal in 0..premise_sources {
        source_schedule.push(
            GeneratedAffineResidualGroupExactConditionSourceLocator::TargetPremise {
                premise_ordinal,
            },
        );
    }
    for guard_ordinal in 0..row_guard_sources {
        source_schedule.push(
            GeneratedAffineResidualGroupExactConditionSourceLocator::RecenteredRowGuard {
                guard_ordinal,
            },
        );
    }
    source_schedule.push(
        GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
            term_ordinal: ready.pivot_term_ordinal(),
        },
    );
    for descent in ready.descent() {
        source_schedule.push(
            GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                rhs_ordinal: descent.rhs_ordinal(),
                term_ordinal: descent.term_ordinal(),
            },
        );
    }
    if source_schedule.len() != source_schedule_entries {
        return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
    }

    let mut hazard_schedule =
        try_vec_with_capacity("exact condition hazard schedule", hazard_locators)?;
    for (hazard_ordinal, hazard) in ready.hazards().iter().enumerate() {
        hazard_schedule.push(GeneratedAffineResidualGroupExactConditionHazardLocator {
            hazard_ordinal,
            rhs_ordinal: hazard.rhs_ordinal(),
            term_ordinal: hazard.term_ordinal(),
            coordinate: hazard.coordinate(),
        });
    }

    let source_schedule_retained_bytes = checked_mul(
        "exact condition source schedule retained bytes",
        source_schedule.capacity(),
        size_of::<GeneratedAffineResidualGroupExactConditionSourceLocator>(),
    )?;
    check_limit(
        "exact condition source schedule retained bytes",
        source_schedule_retained_bytes,
        limits.max_source_schedule_retained_bytes,
    )?;
    let hazard_schedule_retained_bytes = checked_mul(
        "exact condition hazard schedule retained bytes",
        hazard_schedule.capacity(),
        size_of::<GeneratedAffineResidualGroupExactConditionHazardLocator>(),
    )?;
    check_limit(
        "exact condition hazard schedule retained bytes",
        hazard_schedule_retained_bytes,
        limits.max_hazard_schedule_retained_bytes,
    )?;

    let schedule_bytes = checked_add(
        "exact condition-plan retained owned logical bytes",
        source_schedule_retained_bytes,
        hazard_schedule_retained_bytes,
    )?;
    let retained_owned_logical_bytes = checked_add(
        "exact condition-plan retained owned logical bytes",
        outer_shell,
        checked_add(
            "exact condition-plan retained owned logical bytes",
            target_retained,
            schedule_bytes,
        )?,
    )?;
    check_limit(
        "exact condition-plan retained owned logical bytes",
        retained_owned_logical_bytes,
        limits.max_retained_owned_logical_bytes,
    )?;
    let transform_compilation_peak = checked_add(
        "exact condition-plan compilation owned logical peak upper bound",
        outer_shell,
        target_peak,
    )?;
    let compilation_owned_logical_peak_upper_bound =
        transform_compilation_peak.max(retained_owned_logical_bytes);
    check_limit(
        "exact condition-plan compilation owned logical peak upper bound",
        compilation_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    let stats = GeneratedAffineResidualGroupExactConditionPlanStats {
        target_geometry_entries_inspected: geometry_entries,
        premise_sources,
        row_guard_sources,
        coefficient_sources,
        source_schedule_entries,
        hazard_locators,
        identity_target_transform: identity,
        target_transform_retained_owned_logical_bytes: target_retained,
        target_transform_compilation_owned_logical_peak_upper_bound: target_peak,
        source_schedule_retained_bytes,
        hazard_schedule_retained_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    };
    validate_stats(stats, ready)?;
    Ok(PreparedConditionPlan {
        target_transform,
        source_schedule,
        hazard_schedule,
        stats,
    })
}

fn target_transform_is_identity(
    ambient_arity: usize,
    target_offset: &[Integer],
    free_positions: &[usize],
    matrix: &[Integer],
) -> Result<bool, GeneratedAffineResidualGroupExactConditionPlanError> {
    let expected_matrix = checked_mul(
        "exact condition target matrix entries",
        ambient_arity,
        free_positions.len(),
    )?;
    if target_offset.len() != ambient_arity || matrix.len() != expected_matrix {
        return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
    }
    if free_positions.len() != ambient_arity
        || free_positions
            .iter()
            .copied()
            .enumerate()
            .any(|(ordinal, position)| ordinal != position)
        || target_offset
            .iter()
            .any(|value| value.cmp(&Integer::zero()).is_ne())
    {
        return Ok(false);
    }
    for row in 0..ambient_arity {
        for column in 0..ambient_arity {
            let expected = if row == column {
                Integer::one()
            } else {
                Integer::zero()
            };
            if matrix[row * ambient_arity + column].cmp(&expected).is_ne() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn validate_ready_ordinals(
    ready: &GeneratedAffineResidualGroupReadyForConditions,
) -> Result<(), GeneratedAffineResidualGroupExactConditionPlanError> {
    let terms = ready.ready().terms();
    let pivot = ready.pivot_term_ordinal();
    if pivot >= terms.len() || ready.descent().len().checked_add(1) != Some(terms.len()) {
        return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
    }
    let mut expected_rhs = 0usize;
    for (term_ordinal, _) in terms.iter().enumerate() {
        if term_ordinal == pivot {
            continue;
        }
        let descent = ready
            .descent()
            .get(expected_rhs)
            .ok_or(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady)?;
        if descent.rhs_ordinal() != expected_rhs || descent.term_ordinal() != term_ordinal {
            return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
        }
        expected_rhs = expected_rhs.checked_add(1).ok_or(
            GeneratedAffineResidualGroupExactConditionPlanError::ResourceCountOverflow {
                resource: "exact condition RHS ordinal",
            },
        )?;
    }
    let mut previous = None;
    for hazard in ready.hazards() {
        let descent = ready
            .descent()
            .get(hazard.rhs_ordinal())
            .ok_or(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady)?;
        if descent.term_ordinal() != hazard.term_ordinal()
            || hazard.coordinate() >= ready.geometry().ambient_arity()
            || previous.is_some_and(|prior| {
                prior
                    >= (
                        hazard.rhs_ordinal(),
                        hazard.coordinate(),
                        hazard.term_ordinal(),
                    )
            })
        {
            return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
        }
        previous = Some((
            hazard.rhs_ordinal(),
            hazard.coordinate(),
            hazard.term_ordinal(),
        ));
    }
    Ok(())
}

fn target_transform_logical_memory(
    target: &GeneratedAffineResidualGroupExactConditionTargetTransform,
) -> Result<(usize, usize), GeneratedAffineResidualGroupExactConditionPlanError> {
    match target {
        GeneratedAffineResidualGroupExactConditionTargetTransform::Identity { .. } => {
            let bytes = size_of::<GeneratedAffineResidualGroupExactConditionTargetTransform>();
            Ok((bytes, bytes))
        }
        GeneratedAffineResidualGroupExactConditionTargetTransform::Compact(plan) => {
            let enum_padding = size_of::<GeneratedAffineResidualGroupExactConditionTargetTransform>()
                .checked_sub(size_of::<ResidualAffineCompactCompositionPlan>())
                .ok_or(
                    GeneratedAffineResidualGroupExactConditionPlanError::ResourceCountOverflow {
                        resource: "exact condition target-transform enum padding",
                    },
                )?;
            Ok((
                checked_add(
                    "exact condition target-transform retained owned logical bytes",
                    enum_padding,
                    plan.stats().retained_owned_logical_bytes(),
                )?,
                checked_add(
                    "exact condition target-transform compilation owned logical peak upper bound",
                    enum_padding,
                    plan.stats().compilation_owned_logical_peak_upper_bound(),
                )?,
            ))
        }
    }
}

fn condition_plan_outer_shell_bytes()
-> Result<usize, GeneratedAffineResidualGroupExactConditionPlanError> {
    size_of::<GeneratedAffineResidualGroupExactConditionPlan>()
        .checked_sub(size_of::<GeneratedAffineResidualGroupReadyForConditions>())
        .and_then(|bytes| {
            bytes.checked_sub(size_of::<
                GeneratedAffineResidualGroupExactConditionTargetTransform,
            >())
        })
        .ok_or(
            GeneratedAffineResidualGroupExactConditionPlanError::ResourceCountOverflow {
                resource: "exact condition-plan outer shell bytes",
            },
        )
}

fn target_transforms_match(
    left: &GeneratedAffineResidualGroupExactConditionTargetTransform,
    right: &GeneratedAffineResidualGroupExactConditionTargetTransform,
) -> bool {
    match (left, right) {
        (
            GeneratedAffineResidualGroupExactConditionTargetTransform::Identity {
                ambient_arity: left,
            },
            GeneratedAffineResidualGroupExactConditionTargetTransform::Identity {
                ambient_arity: right,
            },
        ) => left == right,
        (
            GeneratedAffineResidualGroupExactConditionTargetTransform::Compact(left),
            GeneratedAffineResidualGroupExactConditionTargetTransform::Compact(right),
        ) => left.manifest() == right.manifest(),
        _ => false,
    }
}

fn validate_stats(
    stats: GeneratedAffineResidualGroupExactConditionPlanStats,
    ready: &GeneratedAffineResidualGroupReadyForConditions,
) -> Result<(), GeneratedAffineResidualGroupExactConditionPlanError> {
    if stats.premise_sources != ready.ready().target_premises().len()
        || stats.row_guard_sources != ready.ready().row_guards().len()
        || stats.coefficient_sources != ready.ready().terms().len()
        || stats.hazard_locators != ready.hazards().len()
        || stats.source_schedule_entries
            != checked_add(
                "exact condition source schedule conservation",
                checked_add(
                    "exact condition source schedule conservation",
                    stats.premise_sources,
                    stats.row_guard_sources,
                )?,
                stats.coefficient_sources,
            )?
        || stats.retained_owned_logical_bytes > stats.compilation_owned_logical_peak_upper_bound
    {
        return Err(GeneratedAffineResidualGroupExactConditionPlanError::MalformedReady);
    }
    Ok(())
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupExactConditionPlanError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupExactConditionPlanError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupExactConditionPlanError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupExactConditionPlanError::ResourceLimit {
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
) -> Result<usize, GeneratedAffineResidualGroupExactConditionPlanError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualGroupExactConditionPlanError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupExactConditionPlanError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualGroupExactConditionPlanError::ResourceCountOverflow { resource },
    )
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;
    use crate::generated_affine_residual_group_exact_session::tests::{
        ExactConditionPlanTestFixture, exact_condition_plan_test_fixture,
    };

    fn compile(
        fixture: ExactConditionPlanTestFixture,
        limits: GeneratedAffineResidualGroupExactConditionPlanLimits,
    ) -> Result<
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactConditionPlan,
        ),
        (
            IntegralFamily,
            ParametricCoefficientContext,
            GeneratedAffineResidualGroupExactSession,
            GeneratedAffineResidualGroupExactConditionPlanFailure,
        ),
    > {
        let ExactConditionPlanTestFixture {
            family,
            context,
            session,
            ready,
        } = fixture;
        match GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family, &context, &session, ready, limits,
        ) {
            Ok(plan) => Ok((family, context, session, plan)),
            Err(failure) => Err((family, context, session, failure)),
        }
    }

    #[test]
    fn identity_plan_retains_pivot_first_deterministic_schedule_and_replays() {
        let fixture =
            exact_condition_plan_test_fixture("exact-condition-plan-identity-schedule", false);
        let (family, context, session, plan) = compile(
            fixture,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        assert_eq!(
            plan.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_CONDITION_PLAN_V1_SCHEMA,
        );
        assert!(plan.target_transform_is_identity());
        assert!(plan.compact_target_transform().is_none());
        assert_eq!(plan.targets_consumed(), 0);
        assert!(!plan.publishes_rule());

        let stats = plan.stats();
        assert_eq!(
            stats.source_schedule_entries(),
            plan.source_schedule().len()
        );
        assert_eq!(stats.hazard_locators(), plan.hazard_schedule().len());
        let pivot_position = stats.premise_sources() + stats.row_guard_sources();
        assert_eq!(
            plan.source_schedule()[pivot_position],
            GeneratedAffineResidualGroupExactConditionSourceLocator::PivotCoefficient {
                term_ordinal: plan.ready().pivot_term_ordinal(),
            },
        );
        for (rhs_ordinal, descent) in plan.ready().descent().iter().enumerate() {
            assert_eq!(
                plan.source_schedule()[pivot_position + 1 + rhs_ordinal],
                GeneratedAffineResidualGroupExactConditionSourceLocator::RhsCoefficient {
                    rhs_ordinal,
                    term_ordinal: descent.term_ordinal(),
                },
            );
        }
        for (ordinal, locator) in plan.hazard_schedule().iter().copied().enumerate() {
            let hazard = &plan.ready().hazards()[ordinal];
            assert_eq!(locator.hazard_ordinal(), ordinal);
            assert_eq!(locator.rhs_ordinal(), hazard.rhs_ordinal());
            assert_eq!(locator.term_ordinal(), hazard.term_ordinal());
            assert_eq!(locator.coordinate(), hazard.coordinate());
        }
        plan.replay(&family, &context, &session).unwrap();
        assert!(format!("{plan:?}").contains("private_ready: \"<redacted>\""));
    }

    #[test]
    fn constrained_compact_plan_replays_exact_geometry_and_rejects_foreign_session() {
        let name = "exact-condition-plan-compact-owner";
        let owner = exact_condition_plan_test_fixture(name, true);
        let foreign = exact_condition_plan_test_fixture(name, true);
        let foreign_session = foreign.session;
        let (family, context, session, plan) = compile(
            owner,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        assert!(!plan.target_transform_is_identity());
        let compact = plan.compact_target_transform().unwrap();
        assert_eq!(compact.ambient_arity(), 3);
        assert_eq!(compact.free_positions(), &[1]);
        plan.replay(&family, &context, &session).unwrap();
        assert!(plan.replay(&family, &context, &foreign_session).is_err());
    }

    #[test]
    fn every_positive_outer_limit_is_exact_and_one_below_recovers_the_owner() {
        let baseline =
            exact_condition_plan_test_fixture("exact-condition-plan-resource-baseline", true);
        let (_, _, _, baseline_plan) = compile(
            baseline,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        let stats = baseline_plan.stats();

        let mut exact = GeneratedAffineResidualGroupExactConditionPlanLimits::default();
        exact.max_target_geometry_entries_inspected = stats.target_geometry_entries_inspected();
        exact.max_premise_sources = stats.premise_sources();
        exact.max_row_guard_sources = stats.row_guard_sources();
        exact.max_coefficient_sources = stats.coefficient_sources();
        exact.max_source_schedule_entries = stats.source_schedule_entries();
        exact.max_hazard_locators = stats.hazard_locators();
        exact.max_source_schedule_retained_bytes = stats.source_schedule_retained_bytes();
        exact.max_hazard_schedule_retained_bytes = stats.hazard_schedule_retained_bytes();
        exact.max_retained_owned_logical_bytes = stats.retained_owned_logical_bytes();
        exact.max_compilation_owned_logical_peak_upper_bound =
            stats.compilation_owned_logical_peak_upper_bound();

        let fixture =
            exact_condition_plan_test_fixture("exact-condition-plan-resource-recovery", true);
        let ExactConditionPlanTestFixture {
            family,
            context,
            session,
            mut ready,
        } = fixture;
        let mut candidates = Vec::new();
        macro_rules! one_below {
            ($field:ident, $value:expr) => {
                if $value > 0 {
                    let mut limits = exact;
                    limits.$field = $value - 1;
                    candidates.push(limits);
                }
            };
        }
        one_below!(
            max_target_geometry_entries_inspected,
            stats.target_geometry_entries_inspected()
        );
        one_below!(max_premise_sources, stats.premise_sources());
        one_below!(max_row_guard_sources, stats.row_guard_sources());
        one_below!(max_coefficient_sources, stats.coefficient_sources());
        one_below!(max_source_schedule_entries, stats.source_schedule_entries());
        one_below!(max_hazard_locators, stats.hazard_locators());
        one_below!(
            max_source_schedule_retained_bytes,
            stats.source_schedule_retained_bytes()
        );
        one_below!(
            max_hazard_schedule_retained_bytes,
            stats.hazard_schedule_retained_bytes()
        );
        one_below!(
            max_retained_owned_logical_bytes,
            stats.retained_owned_logical_bytes()
        );
        one_below!(
            max_compilation_owned_logical_peak_upper_bound,
            stats.compilation_owned_logical_peak_upper_bound()
        );
        assert!(candidates.len() >= 7);

        for limits in candidates {
            let failure = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
                &family, &context, &session, ready, limits,
            )
            .unwrap_err();
            assert!(matches!(
                failure.error(),
                GeneratedAffineResidualGroupExactConditionPlanError::ResourceLimit { .. }
                    | GeneratedAffineResidualGroupExactConditionPlanError::TargetTransform(
                        ResidualUnitAffineCompositionError::ResourceLimit { .. }
                    )
            ));
            let (_, recovered) = failure.into_parts();
            recovered.replay(&family, &context, &session).unwrap();
            ready = recovered;
        }

        let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family, &context, &session, ready, exact,
        )
        .unwrap();
        assert_eq!(plan.stats(), stats);
        plan.replay(&family, &context, &session).unwrap();
    }

    #[test]
    fn caught_boundary_panic_returns_replayable_exact_owner() {
        let ExactConditionPlanTestFixture {
            family,
            context,
            session,
            ready,
        } = exact_condition_plan_test_fixture("exact-condition-plan-panic-recovery", false);
        inject_condition_plan_boundary_panic_for_test();
        let failure = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family,
            &context,
            &session,
            ready,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            &GeneratedAffineResidualGroupExactConditionPlanError::SymbolicaPanic,
        );
        let (_, ready) = failure.into_parts();
        ready.replay(&family, &context, &session).unwrap();
        let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
            &family,
            &context,
            &session,
            ready,
            GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
        )
        .unwrap();
        plan.replay(&family, &context, &session).unwrap();
    }

    #[test]
    fn hazard_locators_are_fixed_width_and_do_not_retain_exact_interval_values() {
        assert_eq!(
            size_of::<GeneratedAffineResidualGroupExactConditionHazardLocator>(),
            4 * size_of::<usize>(),
        );
        let locator = GeneratedAffineResidualGroupExactConditionHazardLocator {
            hazard_ordinal: usize::MAX,
            rhs_ordinal: usize::MAX - 1,
            term_ordinal: usize::MAX - 2,
            coordinate: usize::MAX - 3,
        };
        assert_eq!(locator.hazard_ordinal(), usize::MAX);
        assert_eq!(locator.rhs_ordinal(), usize::MAX - 1);
        assert_eq!(locator.term_ordinal(), usize::MAX - 2);
        assert_eq!(locator.coordinate(), usize::MAX - 3);
    }
}
