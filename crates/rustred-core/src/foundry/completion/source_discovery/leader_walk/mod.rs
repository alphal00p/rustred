//! Proposal-only planning for maximal-orthant leader walks.
//!
//! This module freezes an exact uncovered-partition snapshot into two bounded,
//! deterministic seed waves: every selected maximal-free-dimension box
//! contributes its lower corner and then one separate point raised along each
//! unbounded axis. It only produces translated-source target proposals. A
//! task, a completed wave, or its neutral planning-envelope census is never an
//! exact relation, rule cell, cover update, owner, terminal, or closure
//! certificate. Round-robin fairness applies only inside this finite seed
//! envelope; this is not a fair infinite upward walk.
//!
//! This local planner owns snapshot freezing, seed ordering, checked chart
//! conversion, all-or-error planning caps, and in-memory epoch invalidation.
//! The future execution driver must separately own the shared campaign ledger,
//! worker-result merge, algebraic execution, admission, `BoxCover` delta,
//! geometry rebuild, and repeated-walk fairness. Exact promotion remains behind
//! the existing replay, guard, strict-descent, and owner-cover boundaries. The
//! planner has no API that mutates a [`super::super::UncoveredPartition`] or
//! crosses those boundaries.

mod error;
mod limits;
mod model;
mod plan;
mod requested;

pub(crate) use error::LeaderWalkPlanError;
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use limits::LeaderWalkLimits;
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use model::{
    LeaderWalkDepth, LeaderWalkPlan, LeaderWalkScopePartition, LeaderWalkTask, LeaderWalkTaskKey,
    LeaderWalkWave, PlanningEnvelopeCensus,
};
#[allow(unused_imports)] // Proposal seam awaiting the algebraic execution driver.
pub(crate) use plan::try_plan_maximal_orthant_leader_walk;
#[allow(unused_imports)] // Offline exact-anchor diagnostics consume this bounded plan.
pub(crate) use requested::{
    RequestedDomain, RequestedDomainPlan, RequestedDomainScopePartition, RequestedDomainTask,
    RequestedDomainTaskKey, try_plan_requested_domains,
};

#[cfg(test)]
mod tests;
