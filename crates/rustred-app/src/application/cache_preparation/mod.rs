//! Opt-in preparation policy and finite native workloads. This layer neither
//! generates rules nor owns a coefficient codec or an eviction algorithm.
mod closed;
mod options;
mod runner;
mod workload;

pub use closed::{ClosedApplicationCache, ClosedCachePreparationBackend};
pub use options::{
    CachePreparationCapabilities, CachePreparationOptions, CachePreparationResolvedOptions,
};
pub use runner::{
    CachePreparationBackend, CachePreparationReport, CachePreparationStatus,
    prepare_application_cache,
};
pub use workload::{CachePreparationTarget, CachePreparationWorkload};

#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
