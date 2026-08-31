//! Proposal-only affine-unisolvent sampling inside blind lattice components.
//!
//! For every box of globally maximal free dimension, this planner moves all
//! unbounded coordinates by a caller-selected interior margin and adds the
//! complete nonnegative simplex of offsets through a total-degree ceiling.
//! Bounded coordinates remain at the box's lower endpoint.  The resulting
//! target proposals are suitable for testing or reconstructing polynomial
//! dependence in a later algebraic layer.
//!
//! This module performs lattice scheduling only.  It does not evaluate an IBP,
//! infer polynomial degree, reconstruct a coefficient, admit a rule, mutate a
//! cover, or authorize an owner, terminal, artifact, or closure claim.  A
//! completed schedule therefore has no semantic authority beyond identifying
//! which target proposals belonged to one frozen in-memory geometry epoch.

mod build;
mod canonical;
mod error;
mod execution;
mod freeze;
mod limits;
mod model;
mod plan;
mod resource;
mod simplex;
mod target;

pub(crate) use error::InteriorSimplexPlanError;
#[allow(unused_imports)] // Proposal executor awaiting the completion driver.
pub(crate) use execution::{
    InteriorSimplexBootstrapTelemetry, InteriorSimplexExecutionError,
    InteriorSimplexExecutionLimits, InteriorSimplexExecutionReport,
    InteriorSimplexIterationTelemetry, InteriorSimplexOutcomeTelemetry,
    InteriorSimplexProbeExecutor, InteriorSimplexProbeTelemetry, InteriorSimplexReplayRetention,
    InteriorSimplexRetainedPayloadCensus, InteriorSimplexTaskExecutionReport,
};
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use limits::InteriorSimplexLimits;
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use model::{
    InteriorSimplexPlan, InteriorSimplexScopePartition, InteriorSimplexTask, InteriorSimplexTaskKey,
};
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use plan::try_plan_interior_simplex_samples;

#[cfg(test)]
mod tests;
