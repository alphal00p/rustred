//! Experimental concrete application of source-port candidate formulas.
//!
//! This owner is deliberately NOT a `ClosedArtifact`. It neither replays
//! original-source provenance nor proves a family cover. Each requested
//! integer point is checked against the actual case, whole exceptional
//! conjunctions, inherited source conditions, coefficient denominators, and
//! exact descent order. A missing rule is an error, never an inferred master.
//! Oracle comparisons can test this lane before durable publication succeeds.

mod application;
mod model;
mod preparation;
mod reducer;

pub use model::{CandidateDecomposition, CandidateReductionError, CandidateStatistics};
pub use reducer::CandidateReducer;

#[cfg(test)]
mod tests;
