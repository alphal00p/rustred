//! Opt-in preparation policy and finite native workloads. This layer neither
//! generates rules nor owns a coefficient codec or an eviction algorithm.
mod batch;
mod capture;
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

fn resource_error(error: symbolica::utils::resources::ResourceLimitError) -> crate::AppError {
    crate::AppError::resource(error)
}

fn binary_error(
    error: rustred::persistence::BinaryIoError,
    fallback: crate::AppErrorKind,
) -> crate::AppError {
    match error {
        rustred::persistence::BinaryIoError::Resource(resource) => {
            crate::AppError::resource(resource)
        }
        error => crate::AppError::new(fallback, error.to_string()),
    }
}

#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod generic_tests;
#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod tests;
