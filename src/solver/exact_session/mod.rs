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
pub(crate) use physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupExactPhysicalRowCompiler,
    GeneratedAffineResidualGroupExactPhysicalRowLimits,
};
#[cfg(test)]
pub(crate) use session::GeneratedAffineResidualGroupExactSessionRecenterOutcome;
