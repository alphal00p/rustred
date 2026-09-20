//! Optional trusted-local, single-writer candidate sector storage.
//!
//! A receipt establishes durable storage, not valid algebra or certification.
//! Resume admits structural request/sector bindings without importing State;
//! assembly must import each shard once and check its native family and indexed
//! coefficient contexts before returning any final output. Invalid installed
//! shards are never overwritten or silently scheduled for regeneration.

mod manifest;
mod store;

use std::path::PathBuf;

pub(super) use manifest::CheckpointManifest;
pub(super) use store::CheckpointStore;

/// Optional on-disk sector checkpoints for candidate generation.
///
/// The directory is trusted generated data, exclusively owned while a request
/// runs. Existing candidates remain uncertified. The byte budget counts logical
/// payload lengths (manifest, retained chunks, recognized abandoned staging
/// files and reserved concurrent writes), not filesystem metadata/block overhead
/// or peak memory. Final candidate bundle/algebra limits remain independent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateCheckpointOptions {
    pub directory: PathBuf,
    /// Require and reuse an exactly matching existing campaign manifest.
    pub resume: bool,
    pub max_total_bytes: usize,
}

impl CandidateCheckpointOptions {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            resume: false,
            max_total_bytes: super::MAX_CANDIDATE_BUNDLE_BYTES,
        }
    }
}
