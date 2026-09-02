//! Deterministic probe-local obstruction campaigns.
//!
//! Every admitted finite-field probe starts from an independently recomputed
//! target-unit bootstrap, owns its complete request accumulator, and rebuilds
//! all plan-local evidence after every augmentation. Cross-probe state is
//! restricted to bounded scalar work telemetry; requests, selected rows,
//! samples, hits, and obstructions are never shared as authority.
//!
//! A modular hit is lifted synchronously while its fresh epoch is alive. A
//! no-hit can produce sampled-dual evidence only after exhaustive incidence
//! nomination, complete residual evaluation, and successful fail-closed dual
//! admission. None of the outcomes in this module can install a rule, owner,
//! terminal, artifact, or closure claim.

mod error;
mod limits;
// This crate-private result model is the staged integration boundary for the
// completion driver. Unit-test builds can reach every outcome variant
// directly and therefore lose production's crate-visible consumer assumption;
// suppress only that configuration's premature accessor/payload diagnostics.
#[cfg_attr(test, allow(dead_code))]
mod model;
mod rejection;
mod run;

pub(crate) use error::ProbeLocalSchedulerError;
pub(crate) use limits::ProbeLocalSchedulerLimits;
#[allow(unused_imports)] // Consumed by the staged sector-layer orchestrator.
pub(crate) use model::ProbeLocalOutcomeKind;
pub(crate) use model::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalBudgetStop,
    ProbeLocalIterationDisposition, ProbeLocalIterationRecord, ProbeLocalOutcome,
    ProbeLocalProbeReport, ProbeLocalRejection, ProbeLocalRunCensus, ProbeLocalSchedulerReport,
    ProbeLocalStage, ProbeLocalStall, ProbeLocalStopContext,
};
pub(crate) use rejection::{ProbeLocalRejectionCategory, ProbeLocalRejectionSummary};
#[allow(unused_imports)] // Production caller lands with the sector-layer orchestrator.
pub(crate) use run::ProbeLocalObstructionScheduler;

#[cfg(test)]
mod tests;
