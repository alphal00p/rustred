//! Proposal-only affine-unisolvent sampling inside blind lattice components.
//!
//! For every box at either the globally maximal or one explicitly requested
//! positive free dimension, this planner enumerates the complete Cartesian
//! product of its finite coordinates, moves every unbounded coordinate by a
//! caller-selected positive interior margin, and adds the complete
//! nonnegative simplex of offsets through a total-degree ceiling. The product
//! and retained result are preflighted before task
//! construction; finite assignments are streamed by mixed-radix ordinal and
//! are never materialized as a separate combinatorial table. Canonical box
//! rounds are flattened in linear work, and an ordered active frontier visits
//! each live finite assignment exactly once instead of scanning the rectangle
//! formed by the largest product and every box. The resulting target
//! proposals are suitable for testing or reconstructing polynomial dependence
//! in a later algebraic layer.
//!
//! This module performs lattice scheduling only.  It does not evaluate an IBP,
//! infer polynomial degree, reconstruct a coefficient, admit a rule, mutate a
//! cover, or authorize an owner, terminal, artifact, or closure claim.  A
//! completed schedule therefore has no semantic authority beyond identifying
//! which target proposals belonged to one frozen in-memory geometry epoch.
//!
//! Exact-dimension selection filters the canonical boxes already present in
//! the supplied partition. It does not synthesize lower-dimensional boundary
//! faces inside a box of higher free dimension; a separate proposal-only face
//! planner must sample those faces without mutating the exact cover geometry.

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
    InteriorSimplexFreeDimensionSelection, InteriorSimplexPlan, InteriorSimplexScopePartition,
    InteriorSimplexTask, InteriorSimplexTaskKey,
};
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use plan::{
    try_plan_interior_simplex_samples, try_plan_interior_simplex_samples_at_free_dimension,
};

#[cfg(test)]
mod tests;
