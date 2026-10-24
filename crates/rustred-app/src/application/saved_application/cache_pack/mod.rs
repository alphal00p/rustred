//! Explicit trusted-generated, content-bound cache packs. A reader retains a
//! bounded chunk-range directory and compact persisted negative key filters,
//! with optional verified exact-key metadata for legacy chunks,
//! never retained coefficient payloads.
//! A writer publishes complete immutable chunks through atomic checkpoints.
//! This module does not grant entry admission or bypass native cache budgets.
//! Chunks contain one bounded gzip member around the native state-aware codec.
//! A lookup retains at most one encoded and one decoded chunk, plus the native
//! record decoder's bounded table/output allocations. These are I/O envelopes,
//! not a bound on Symbolica scratch or process RSS. Overlapping immutable runs
//! remain possible; cumulative lookup limits refuse excessive read/decode work.
mod bundle;
mod compression;
mod index;
mod quota;
mod reader;
#[cfg(test)]
mod tests;
mod writer;

use crate::AppError;
use rustred::persistence::application_cache::ApplicationCacheIoLimits;

pub use bundle::{
    SavedApplicationCacheBundle, SavedApplicationCacheBundleWriter,
    SavedApplicationCacheCopyStatistics,
};
pub use reader::{SavedApplicationCacheLookupOutcome, SavedApplicationCachePack};
pub use writer::SavedApplicationCacheWriter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedApplicationCachePackLimits {
    /// Enable the count and cumulative lookup-work diagnostics below.
    /// Ordinary execution is governed by explicit bytes and ambient resources.
    pub diagnostics_enabled: bool,
    pub chunk: ApplicationCacheIoLimits,
    pub max_manifest_bytes: usize,
    pub max_directory_owned_bytes: usize,
    pub max_chunks: usize,
    pub max_runs: usize,
    pub max_namespaces: usize,
    /// Compressed bytes per chunk, independently of `chunk.binary`'s decoded cap.
    pub max_encoded_chunk_bytes: usize,
    /// Candidate chunks allowed per lookup across overlapping immutable runs.
    pub max_lookup_chunks: usize,
    /// Prospective compressed reads per lookup, including one overrun probe per
    /// chunk. The bounded manifest is read once at open, not during lookups.
    pub max_lookup_encoded_bytes: u64,
    /// Prospective decoded bytes per lookup, including one EOF/overrun probe per
    /// chunk. Both byte grants are checked before reading a candidate chunk.
    pub max_lookup_decoded_bytes: u64,
    /// Files physically present, including unreferenced interrupted chunks.
    /// This is independent of the caller's retained coefficient/RAM allowance.
    pub max_disk_bytes: u64,
}
impl Default for SavedApplicationCachePackLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            chunk: Default::default(),
            max_manifest_bytes: usize::MAX,
            max_directory_owned_bytes: usize::MAX,
            max_chunks: 32768,
            max_runs: 4096,
            max_namespaces: 128,
            max_encoded_chunk_bytes: usize::MAX,
            max_lookup_chunks: 8,
            max_lookup_encoded_bytes: 512 * 1024 * 1024,
            max_lookup_decoded_bytes: 512 * 1024 * 1024,
            max_disk_bytes: u64::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationCachePackStatistics {
    pub runs: usize,
    pub chunks: usize,
    /// Stored records, including shadowed keys in earlier immutable runs.
    pub records: usize,
    pub referenced_chunk_bytes: u64,
    pub manifest_bytes: usize,
    /// Upper census of decoded directory structs, owned key/identity allocations
    /// and optional exact chunk-key metadata. No coefficient payload lives here.
    pub directory_owned_bytes: usize,
    pub chunks_read: usize,
    pub chunk_bytes_read: u64,
    pub largest_chunk_read: usize,
    pub decoded_chunk_bytes: u64,
    pub largest_decoded_chunk: usize,
    /// Lifetime prospective charges, including probes and failed operations.
    pub lookup_encoded_bytes_charged: u64,
    pub lookup_decoded_bytes_charged: u64,
    pub lookup_limit_refusals: usize,
    pub runs_probed: usize,
    /// Trusted persisted fingerprints or previously verified exact chunk keys
    /// rejected a range-only candidate without a file read, decompression, or
    /// native coefficient import. A fingerprint match never authorizes a hit.
    pub key_index_skips: usize,
    pub hits: usize,
    pub misses: usize,
}

fn fail(error: impl std::fmt::Display) -> crate::AppError {
    crate::AppError::input(format!("application cache pack: {error}"))
}

pub(super) fn resource_checkpoint() -> Result<(), AppError> {
    symbolica::utils::resources::checkpoint().map_err(AppError::resource)
}

pub(super) fn resource_allocation(bytes: usize) -> Result<(), AppError> {
    symbolica::utils::resources::check_allocation(u64::try_from(bytes).map_err(fail)?)
        .map_err(AppError::resource)
}

pub(super) fn io_error(error: std::io::Error) -> AppError {
    if let Some(inner) = error.get_ref() {
        if let Some(error) = inner.downcast_ref::<symbolica::utils::resources::ResourceLimitError>()
        {
            return AppError::resource(error.clone());
        }
        if let Some(error) = inner.downcast_ref::<AppError>() {
            return error.clone();
        }
    }
    fail(error)
}

pub(super) fn encode_error(error: bincode::error::EncodeError) -> AppError {
    match error {
        bincode::error::EncodeError::Io { inner, .. } => io_error(inner),
        error => fail(error),
    }
}

pub(super) fn binary_error(error: rustred::persistence::BinaryIoError) -> AppError {
    match error {
        rustred::persistence::BinaryIoError::Resource(error) => AppError::resource(error),
        rustred::persistence::BinaryIoError::Limit { .. } => AppError::limit(error.to_string()),
        error => fail(error),
    }
}
