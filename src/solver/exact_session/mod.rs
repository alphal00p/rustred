//! Exact generated-affine solve-session foundations.
//!
//! This boundary owns topology-neutral physical coordinates, immutable solve
//! plans, sealed exact rows, authority-free GMP recentering, and the
//! transactional database, target catalog, session owner/state machine, and
//! native scaling telemetry. Ready/WhenBad materialization and publication
//! remain outside this deliberately narrow crate-private facade until their
//! ownership cycles are inverted in later restructuring tranches.

mod database;
mod physical_key;
mod physical_row;
mod plan;
mod recenter;
mod session;
mod targets;
mod telemetry;

pub(crate) use session::{
    ApplicableRuleHandle, CommittedPublicationDomainView, CommittedPublicationEventHandle,
    CommittedPublicationEventView, CommittedPublicationLeafView, CommittedPublicationPredicateView,
    ExceptionalResidualHandle, ExceptionalResidualKind, GeneratedAffineResidualGroupExactSession,
    GeneratedAffineResidualGroupExactSessionDatabaseCapability,
    GeneratedAffineResidualGroupExactSessionError, GeneratedAffineResidualGroupExactSessionLimits,
    GeneratedAffineResidualGroupExactSessionRecenterReady,
    GeneratedAffineResidualGroupExactSessionRecenterStats, PublicationReceipt,
};

pub(crate) use physical_key::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKey,
    GeneratedAffineResidualGroupPhysicalKeyComparisonComponent,
    GeneratedAffineResidualGroupPhysicalKeyComparisonWitness,
    GeneratedAffineResidualGroupPhysicalKeyError, GeneratedAffineResidualGroupPhysicalKeyLimits,
};
pub(crate) use plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanError,
    GeneratedAffineResidualGroupSolvePlanLimits, GeneratedAffineResidualGroupSolveTargetLocator,
};
pub(crate) use recenter::{integer_bits, prospective_integer_heap_bytes};

#[cfg(test)]
pub(crate) use physical_key::{
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA,
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V3_SCHEMA,
};
#[cfg(test)]
pub(crate) use physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupExactPhysicalRowCompiler,
    GeneratedAffineResidualGroupExactPhysicalRowLimits,
};
#[cfg(test)]
pub(crate) use plan::GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA;
#[cfg(test)]
pub(crate) use session::{
    GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_SESSION_V3_SCHEMA,
    GeneratedAffineResidualGroupExactSessionEventStats,
    GeneratedAffineResidualGroupExactSessionRecenterOutcome,
};
#[cfg(test)]
pub(crate) use targets::{
    GENERATED_AFFINE_RESIDUAL_GROUP_EXACT_TARGET_CATALOG_V3_SCHEMA,
    GeneratedAffineResidualGroupExactTargetError,
};
#[cfg(test)]
pub(crate) use telemetry::NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA;

#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) use super::session::tests::{
        ExactConditionPlanTestFixture, exact_condition_plan_test_fixture,
        exact_condition_plan_test_fixture_in_sector,
        exact_condition_plan_test_fixture_in_sector_with_session_limits,
    };
}
