//! Streaming potential successors of installed parametric rule domains.
//!
//! This is an all-rule union, not first-applicable dispatch, source search or a
//! coverage certificate. Exact native guard references accompany every box;
//! boxes alone are only prefilters. Earlier formulas/terminals and coefficient
//! cancellations can remove these conservative obligations.

mod applied;
mod guarded;
mod matching;
mod model;
mod scan;

pub use guarded::{
    OwnerGuardedDomain, OwnerGuardedError, OwnerGuardedEvent, OwnerGuardedFailure,
    OwnerGuardedImage, OwnerGuardedLimits, OwnerGuardedResidualKind, OwnerGuardedStats,
    OwnerGuardedSuccessor,
};

pub use applied::{
    OwnerAppliedCellRefinement, OwnerAppliedError, OwnerAppliedEvent, OwnerAppliedFailure,
    OwnerAppliedLimits, OwnerAppliedNonzero, OwnerAppliedProblem, OwnerAppliedProblemKind,
    OwnerAppliedStats, OwnerAppliedSuccessor,
};

pub use matching::{
    OwnerDomainMatchDisposition, OwnerDomainMatchError, OwnerDomainMatchFailure,
    OwnerDomainMatchLimits, OwnerDomainMatchPiece, OwnerDomainMatchStats, OwnerDomainPredicate,
    OwnerDomainRefinementAxes,
};

pub use model::{
    OwnerSuccessorError, OwnerSuccessorFailure, OwnerSuccessorLimits, OwnerSuccessorRegion,
    OwnerSuccessorStats, OwnerSuccessorTransition,
};

#[cfg(test)]
mod tests;
