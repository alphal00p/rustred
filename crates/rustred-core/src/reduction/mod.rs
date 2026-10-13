//! Deterministic application of sealed closing artifacts.
//!
//! [`Reducer`] is independent of loop count and topology: it consumes ordered
//! guarded rules and explicit terminals from a sealed
//! [`crate::foundry::artifact::ClosedArtifact`]. Installed unit-mass `K = 1`
//! and `K = 3` artifacts exercise ordinary rules, guarded exceptional cells,
//! exact symmetry routing, and immutable lower-family factorization.

pub(crate) mod cache_policy;
mod error;
pub(crate) mod guard_union;
pub(crate) mod guarded_cache;
mod model;
mod reducer;
pub mod terminal_normalization;
pub mod terminal_relations;

pub use cache_policy::CompletedCachePolicy;
pub use error::ReductionError;
pub use guarded_cache::{
    ClosedUnitMassProgram, GuardedCacheBacking, GuardedCacheBackingError,
    GuardedCacheBackingErrorKind, GuardedCacheError, GuardedRootCache, GuardedRootCacheStatistics,
    GuardedRootExportError, GuardedRootExportStatistics, GuardedRootResult, GuardedRootSource,
};
pub use model::{
    HomogeneousMasterCoefficient, HomogeneousMasterDecomposition, MasterDecomposition,
    ReductionLimits, ReductionStatistics,
};
pub use reducer::Reducer;
pub(crate) use reducer::{
    CacheCensus, CacheWeight, GuardedConditionBudget, ReductionRequest, SharedCacheBudget,
    accumulate_master_in_request, coefficient_cache_weight, convolve_factor_expansion_in_request,
};

#[cfg(test)]
mod tests;
