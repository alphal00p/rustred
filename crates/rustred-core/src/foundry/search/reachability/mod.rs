//! Exact reachability discovery on a finite set of concrete integral keys.
//!
//! This module applies ordered [`RuleCell`](crate::foundry::cell::RuleCell)
//! semantics exactly at every key it actually visits. It is deliberately a
//! **bounded discovery tool**, not a proof that an infinite lattice domain is
//! closed. A closing artifact still needs a symbolic domain partition and an
//! independent verifier that discharges every branch.

mod discover;
mod error;
mod limits;
mod model;

pub use error::ReachabilityError;
pub use limits::ReachabilityLimits;
pub use model::{
    ReachabilityDependency, ReachabilityDisposition, ReachabilityFrontier, ReachabilityNode,
    ReachabilityPlanner, ReachabilityRuleApplication, ReachabilityStatistics, ReachabilityTerminal,
    ReachabilityTerminalKind, ReachabilityTerminalProvider,
};

#[cfg(test)]
mod tests;
