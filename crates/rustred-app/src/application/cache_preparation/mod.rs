//! Opt-in preparation policy and finite native workloads. This layer neither
//! generates rules nor owns a coefficient codec or an eviction algorithm.
mod batch;
mod closed;
mod export;
mod options;
mod runner;
mod workload;

pub use closed::{ClosedApplicationCache, ClosedCachePreparationBackend};
pub use export::{CacheSaveOptions, CacheSaveReport};
pub use options::{
    CachePreparationCapabilities, CachePreparationOptions, CachePreparationResolvedOptions,
};
pub use runner::{
    CachePreparationBackend, CachePreparationReport, CachePreparationStatus,
    prepare_application_cache,
};
pub use workload::{
    CachePreparationPrimerBudget, CachePreparationTarget, CachePreparationWorkload,
};

#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
