//! Deterministic bounded lattice searches and concrete dependency discovery.
//!
//! [`SectorSearchDiamond`] enumerates one exact-sector neighborhood, while
//! [`ReachabilityPlanner`] follows exact RuleCell dependencies from a finite
//! root set. Neither finite search is an infinite-domain closure proof.

mod build;
mod error;
mod limits;
mod model;
mod reachability;

pub use error::SectorSearchError;
pub use limits::SectorSearchLimits;
pub use model::SectorSearchDiamond;
pub use reachability::{
    ReachabilityDependency, ReachabilityDisposition, ReachabilityError, ReachabilityFrontier,
    ReachabilityLimits, ReachabilityNode, ReachabilityPlanner, ReachabilityRuleApplication,
    ReachabilityStatistics, ReachabilityTerminal, ReachabilityTerminalKind,
    ReachabilityTerminalProvider,
};

#[cfg(test)]
mod tests;
