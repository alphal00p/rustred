//! Deterministic application of sealed closing artifacts.
//!
//! [`Reducer`] is independent of loop count and topology: it consumes ordered
//! guarded rules and explicit terminals from a sealed
//! [`crate::foundry::artifact::ClosedArtifact`]. The only artifact installer
//! currently completed is the generated one-loop vacuum preset, so this
//! generic runtime does not imply two-loop closure yet.

mod error;
mod model;
mod reducer;

pub use error::ReductionError;
pub use model::{
    HomogeneousMasterCoefficient, HomogeneousMasterDecomposition, MasterDecomposition,
    ReductionLimits, ReductionStatistics,
};
pub use reducer::Reducer;

#[cfg(test)]
mod tests;
