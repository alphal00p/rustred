//! Experimental concrete application of source-port candidate formulas.
//!
//! This owner is deliberately NOT a `ClosedArtifact`. It neither replays
//! original-source provenance nor proves a family cover. Each requested
//! integer point is checked against the actual case, whole exceptional
//! conjunctions, inherited source conditions, coefficient denominators, and
//! exact descent order. A missing rule is an error, never an inferred master.
//! Oracle comparisons can test this lane before durable publication succeeds.

mod application;
mod cache;
mod evaluator;
mod model;
mod owners;
mod preparation;
mod reducer;
mod routed;
mod terminal_aliases;
mod terminal_normalization;
mod trace;

pub use model::{
    CandidateCacheRepresentation, CandidateDecomposition, CandidateReachabilityReport,
    CandidateReductionError, CandidateStatistics,
};
pub use owners::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
};
pub use reducer::CandidateReducer;
pub use routed::{
    CandidateOwnerRoute, CandidateRoutedError, CandidateRoutedFrontier,
    CandidateRoutedFrontierReason, CandidateRoutedTraceReport, RoutedCandidateLimits,
    RoutedCandidateReducer,
};
pub use trace::{CandidateTraceLimits, CandidateTraceReport};

#[cfg(test)]
mod owner_test_support;
#[cfg(test)]
mod tests;
