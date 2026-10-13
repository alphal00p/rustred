//! Opt-in preparation policy and finite native workloads. This layer neither
//! generates rules nor owns a coefficient codec or an eviction algorithm.
mod batch;
mod export;
mod options;
mod program;
mod runner;
mod session;
mod workload;

pub use export::{CacheSaveMode, CacheSaveOptions, CacheSaveReport};
pub use options::{
    CachePreparationCapabilities, CachePreparationOptions, CachePreparationResolvedOptions,
};
pub use program::NativeCacheProgram;
pub use runner::{
    CachePreparationBackend, CachePreparationReport, CachePreparationStatus,
    prepare_application_cache,
};
pub use session::{
    CacheLookupBudgetPolicy, CandidateApplicationCache, CandidateCachePreparationBackend,
    ClosedApplicationCache, ClosedCachePreparationBackend, NativeApplicationCache,
    NativeCachePreparationBackend,
};
pub use workload::{
    CachePreparationPrimerBudget, CachePreparationTarget, CachePreparationWorkload,
};

#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod generic_tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
