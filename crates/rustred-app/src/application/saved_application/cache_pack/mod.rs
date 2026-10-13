//! Explicit trusted-generated, content-bound cache packs. A reader retains a
//! bounded chunk-range directory, never every entry key or coefficient payload.
//! A writer publishes complete immutable chunks through atomic checkpoints.
//! This module does not grant entry admission or bypass native cache budgets.
mod bundle;
mod index;
mod quota;
mod reader;
#[cfg(test)]
mod tests;
mod writer;

use rustred::persistence::application_cache::ApplicationCacheIoLimits;

pub use bundle::{SavedApplicationCacheBundle, SavedApplicationCacheBundleWriter};
pub use reader::SavedApplicationCachePack;
pub use writer::SavedApplicationCacheWriter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedApplicationCachePackLimits {
    pub chunk: ApplicationCacheIoLimits,
    pub max_manifest_bytes: usize,
    pub max_directory_owned_bytes: usize,
    pub max_chunks: usize,
    pub max_runs: usize,
    pub max_namespaces: usize,
    /// Files physically present, including unreferenced interrupted chunks.
    /// This is independent of the caller's retained coefficient/RAM allowance.
    pub max_disk_bytes: u64,
}
impl Default for SavedApplicationCachePackLimits {
    fn default() -> Self {
        Self {
            chunk: Default::default(),
            max_manifest_bytes: 16 * 1024 * 1024,
            max_directory_owned_bytes: 32 * 1024 * 1024,
            max_chunks: 32768,
            max_runs: 4096,
            max_namespaces: 128,
            max_disk_bytes: 32_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationCachePackStatistics {
    pub chunks: usize,
    /// Stored records, including shadowed keys in earlier immutable runs.
    pub records: usize,
    pub referenced_chunk_bytes: u64,
    pub manifest_bytes: usize,
    /// Upper census of decoded directory structs and their owned key/identity
    /// allocations. No coefficient or complete per-entry index lives here.
    pub directory_owned_bytes: usize,
    pub chunks_read: usize,
    pub chunk_bytes_read: u64,
    pub largest_chunk_read: usize,
    pub hits: usize,
    pub misses: usize,
}

fn fail(error: impl std::fmt::Display) -> crate::AppError {
    crate::AppError::input(format!("application cache pack: {error}"))
}
