//! Deterministic application of sealed closing artifacts.
//!
//! [`Reducer`] is independent of loop count and topology: it consumes ordered
//! guarded rules and explicit terminals from a sealed
//! [`crate::foundry::artifact::ClosedArtifact`]. Installed unit-mass `K = 1`
//! and `K = 3` artifacts exercise ordinary rules, guarded exceptional cells,
//! exact symmetry routing, and immutable lower-family factorization.

mod error;
mod model;
mod reducer;

pub use error::ReductionError;
pub use model::{
    HomogeneousMasterCoefficient, HomogeneousMasterDecomposition, MasterDecomposition,
    ReductionLimits, ReductionStatistics,
};
pub use reducer::Reducer;
pub(crate) use reducer::{
    CacheCensus, CacheWeight, ReductionRequest, SharedCacheBudget, accumulate_master_in_request,
    coefficient_cache_weight, convolve_factor_expansion_in_request,
};

#[cfg(test)]
mod tests;
