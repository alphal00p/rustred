//! Proposal-only simplex sampling on exact lower faces of uncovered boxes.
//!
//! For every input box at one explicitly selected parent free dimension, the
//! planner enumerates every lexicographic subset of unbounded axes at one
//! exact boundary codimension. Selected axes are pinned at their lower
//! endpoints. A positive-margin complete simplex is placed only on the
//! remaining unbounded axes, while the original finite-axis Cartesian product
//! is retained in full. The all-pinned case is represented by an explicit
//! vertex profile rather than by an empty interior-simplex convention.
//!
//! Faces are proposal geometry only. They never become boxes in the exact
//! uncovered partition, and neither a task nor a completed plan can authorize
//! a relation, owner, terminal, artifact, master, exhaustion, or closure
//! claim. Exact replay, descent, guards, predecessor ownership, and owner-cover
//! compilation remain separate downstream boundaries.

mod build;
mod canonical;
mod combinatorics;
mod error;
mod freeze;
mod limits;
mod model;
mod plan;
mod preflight;
mod resource;
mod target;

pub(crate) use error::BoundarySimplexPlanError;
#[allow(unused_imports)] // Proposal seam awaiting the campaign adapter.
pub(crate) use limits::BoundarySimplexLimits;
#[allow(unused_imports)] // Proposal seam awaiting the campaign adapter.
pub(crate) use model::{
    BoundarySimplexPlan, BoundarySimplexSamplingProfile, BoundarySimplexScopePartition,
    BoundarySimplexTask, BoundarySimplexTaskKey,
};
#[allow(unused_imports)] // Proposal seam awaiting the campaign adapter.
pub(crate) use plan::try_plan_boundary_simplex_samples;

#[cfg(test)]
mod tests;
