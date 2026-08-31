//! Serial execution of one frozen interior-simplex proposal plan.
//!
//! Every task receives an independently constructed probe-local scheduler.
//! The only shared inputs are immutable ordinary source rows, generator state,
//! owner snapshot, ordering policy, and declared finite-field probes.  Results
//! are retained in canonical task order.  Canonical ordinals permit a future
//! executor to restore worker completion order, but this implementation makes
//! no parallel-execution claim.
//!
//! Bootstrap support and scheduler outcomes remain discovery telemetry.  This
//! seam cannot promote a replay, mutate a cover, declare exhaustion, install a
//! terminal, publish an artifact, or establish closure.

mod bootstrap;
mod compact;
mod error;
mod limits;
mod model;
mod resource;
mod run;

pub(crate) use error::InteriorSimplexExecutionError;
pub(crate) use limits::InteriorSimplexExecutionLimits;
pub(crate) use model::{
    InteriorSimplexBootstrapTelemetry, InteriorSimplexExecutionReport,
    InteriorSimplexIterationTelemetry, InteriorSimplexOutcomeTelemetry,
    InteriorSimplexProbeTelemetry, InteriorSimplexReplayRetention,
    InteriorSimplexRetainedPayloadCensus, InteriorSimplexTaskExecutionReport,
};
pub(crate) use run::InteriorSimplexProbeExecutor;

#[cfg(test)]
mod tests;
