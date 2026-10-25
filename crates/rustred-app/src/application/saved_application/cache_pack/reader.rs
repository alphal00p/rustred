use rustred::algebra::CoefficientContext;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheChunk, ApplicationCacheKey, ApplicationCacheRecord,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::*;
use crate::AppError;

mod key_index;
mod visit;
use key_index::KeyIndex;

/// A lookup grant can be exhausted without establishing presence or absence.
/// This is deliberately distinct from both a miss and a corrupt/invalid pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedApplicationCacheLookupOutcome {
    Hit(ApplicationCacheRecord),
    Absent,
    /// The next candidate read/decode would exceed configured cumulative work.
    /// No part of that operation was performed. Already spent work stays in
    /// the reader's counters; an optional-cache adapter may explicitly choose
    /// native computation instead, without restarting its reduction request.
    WorkBudgetExhausted,
}

/// An exact committed native index, published separately from its append-only
/// chunk store. The sidecar contains the original native index bytes, including
/// their existing framing/digest; it is not a second cache representation.
/// Callers persist this small descriptor and keep the original chunk directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePackSnapshot {
    pub manifest_path: PathBuf,
    pub generation: u64,
    pub manifest_bytes: u64,
    pub manifest_digest: [u8; 32],
}

pub(super) fn freeze_committed_index(
    directory: &Path,
    expected: &ApplicationCacheBinding,
    sidecar: &Path,
    limits: SavedApplicationCachePackLimits,
) -> Result<NativePackSnapshot, AppError> {
    let bytes = index::read_bounded(&directory.join(index::MANIFEST), limits.max_manifest_bytes)?;
    let (manifest, _) = index::decode(&bytes, expected, limits)?;
    publish_snapshot(directory, &bytes, manifest.generation, sidecar, limits)
}

fn publish_snapshot(
    directory: &Path,
    bytes: &[u8],
    generation: u64,
    sidecar: &Path,
    limits: SavedApplicationCachePackLimits,
) -> Result<NativePackSnapshot, AppError> {
    if bytes.len() > limits.max_manifest_bytes {
        return Err(AppError::limit("snapshot index exceeds manifest allowance"));
    }
    let name = sidecar
        .file_name()
        .ok_or_else(|| fail("snapshot index path has no file name"))?;
    let parent = sidecar
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(fail)?;
    let directory = directory.canonicalize().map_err(fail)?;
    if parent.starts_with(&directory) {
        return Err(fail(
            "snapshot index must be outside the cache pack directory",
        ));
    }
    let path = parent.join(name);
    let snapshot = NativePackSnapshot {
        manifest_path: path.clone(),
        generation,
        manifest_bytes: u64::try_from(bytes.len()).map_err(fail)?,
        manifest_digest: *blake3::hash(bytes).as_bytes(),
    };
    // The caller owns the external sidecar's storage allowance. Its size is
    // independently bounded by max_manifest_bytes; source quota/chunks do not
    // change. A late sync error remains Err even if the sidecar was installed.
    crate::application::atomic_file::write_file_atomically_resource(&path, bytes, false)?;
    Ok(snapshot)
}

fn retain_index_bytes(
    bytes: Vec<u8>,
    statistics: &mut SavedApplicationCachePackStatistics,
    limits: SavedApplicationCachePackLimits,
) -> Result<Arc<Vec<u8>>, AppError> {
    // The raw native index is metadata, shared by forks. Charge its retained
    // buffer and Arc/Vec allocation alongside the decoded range directory.
    let header = std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>();
    let owned = bytes
        .capacity()
        .checked_add(header)
        .ok_or_else(|| fail("snapshot metadata size overflow"))?;
    let total = statistics
        .directory_owned_bytes
        .checked_add(owned)
        .ok_or_else(|| fail("cache directory size overflow"))?;
    if total > limits.max_directory_owned_bytes {
        return Err(AppError::limit(
            "cache native index retention exceeds directory allowance",
        ));
    }
    resource_allocation(header)?;
    statistics.directory_owned_bytes = total;
    Ok(Arc::new(bytes))
}

/// Immutable on-disk snapshot plus local I/O statistics. Persisted compact key
/// fingerprints (or lazily retained exact keys for legacy chunks) reject known
/// misses within the directory-memory allowance. Such skips use the trusted
/// authenticated immutable snapshot; they do not
/// reread files to detect later external mutation. Possible hits still perform
/// normal integrity and native validation. Every lookup drops its encoded
/// chunk/native decode table.
pub struct SavedApplicationCachePack {
    directory: PathBuf,
    manifest: Arc<index::Manifest>,
    raw_manifest: Arc<Vec<u8>>,
    limits: SavedApplicationCachePackLimits,
    statistics: SavedApplicationCachePackStatistics,
    key_index: KeyIndex,
    frozen: bool,
}
impl SavedApplicationCachePack {
    /// Explicit trusted-native import. Binding equality and content hashes are
    /// necessary integrity checks, not a proof of supplied reduction identities.
    pub fn open_generated(
        directory: &Path,
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        resource_checkpoint()?;
        let bytes =
            index::read_bounded(&directory.join(index::MANIFEST), limits.max_manifest_bytes)?;
        let (manifest, mut statistics) = index::decode(&bytes, expected, limits)?;
        let raw_manifest = retain_index_bytes(bytes, &mut statistics, limits)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest: Arc::new(manifest),
            raw_manifest,
            limits,
            statistics,
            key_index: KeyIndex::default(),
            frozen: false,
        })
    }
    /// Open exactly a captured committed index while its original store keeps
    /// appending. No latest-index read, chunk copy or coefficient import occurs.
    /// Integrity checks retain the same trusted-generated native-data boundary.
    pub fn open_snapshot(
        directory: &Path,
        expected: &ApplicationCacheBinding,
        snapshot: &NativePackSnapshot,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        let length = usize::try_from(snapshot.manifest_bytes).map_err(fail)?;
        if length > limits.max_manifest_bytes {
            return Err(AppError::limit("snapshot index exceeds manifest allowance"));
        }
        let bytes = index::read_bounded(&snapshot.manifest_path, length)?;
        if bytes.len() != length || *blake3::hash(&bytes).as_bytes() != snapshot.manifest_digest {
            return Err(fail("snapshot index digest or length mismatch"));
        }
        let (manifest, mut statistics) = index::decode(&bytes, expected, limits)?;
        if manifest.generation != snapshot.generation {
            return Err(fail("snapshot index generation mismatch"));
        }
        let raw_manifest = retain_index_bytes(bytes, &mut statistics, limits)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest: Arc::new(manifest),
            raw_manifest,
            limits,
            statistics,
            key_index: KeyIndex::default(),
            frozen: true,
        })
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.manifest.binding
    }
    pub fn generation(&self) -> u64 {
        self.manifest.generation
    }
    pub fn statistics(&self) -> SavedApplicationCachePackStatistics {
        self.statistics
    }
    /// Persist this reader's exact captured native index, even if the mutable
    /// latest index has changed or disappeared. Only retained metadata is copied;
    /// chunks stay in the original directory and are verified lazily on lookup.
    pub fn freeze_snapshot(&self, sidecar: &Path) -> Result<NativePackSnapshot, AppError> {
        publish_snapshot(
            &self.directory,
            &self.raw_manifest,
            self.generation(),
            sidecar,
            self.limits,
        )
    }
    pub(super) fn captured_snapshot(&self) -> (&Path, &index::Manifest) {
        (&self.directory, &self.manifest)
    }
    /// Authenticate a newer committed directory without decoding coefficients.
    /// Intended for explicit durable-checkpoint boundaries, not each lookup.
    /// On failure the previous snapshot remains usable. Successful refreshes
    /// replace only directory metadata/census: cumulative payload-I/O, lookup
    /// charges and hit/miss counters are preserved, not reset. Index reads are
    /// separately bounded by `max_manifest_bytes`, as at initial open.
    pub fn refresh(&mut self) -> Result<(), AppError> {
        if self.frozen {
            return Err(fail(
                "an immutable cache snapshot cannot refresh to the latest index",
            ));
        }
        let bytes = index::read_bounded(
            &self.directory.join(index::MANIFEST),
            self.limits.max_manifest_bytes,
        )?;
        let (manifest, mut directory) = index::decode(&bytes, self.binding(), self.limits)?;
        if manifest.bundle_managed != self.manifest.bundle_managed
            || manifest.generation < self.manifest.generation
            || manifest.generation == self.manifest.generation && manifest != *self.manifest
        {
            return Err(fail(
                "cache refresh changed ownership or rewound/mutated its generation",
            ));
        }
        if manifest == *self.manifest {
            return Ok(());
        }
        let raw_manifest = retain_index_bytes(bytes, &mut directory, self.limits)?;
        let mut statistics = self.statistics;
        statistics.runs = directory.runs;
        statistics.chunks = directory.chunks;
        statistics.records = directory.records;
        statistics.referenced_chunk_bytes = directory.referenced_chunk_bytes;
        statistics.manifest_bytes = directory.manifest_bytes;
        statistics.directory_owned_bytes = directory.directory_owned_bytes;
        self.manifest = Arc::new(manifest);
        self.raw_manifest = raw_manifest;
        self.key_index = KeyIndex::default();
        self.statistics = statistics;
        Ok(())
    }
    /// Share only immutable directory metadata; no coefficient/route cloning.
    pub fn fork_reader(&self) -> Self {
        let mut statistics = self.statistics;
        statistics.chunks_read = 0;
        statistics.chunk_bytes_read = 0;
        statistics.largest_chunk_read = 0;
        statistics.decoded_chunk_bytes = 0;
        statistics.largest_decoded_chunk = 0;
        statistics.lookup_encoded_bytes_charged = 0;
        statistics.lookup_decoded_bytes_charged = 0;
        statistics.lookup_limit_refusals = 0;
        statistics.runs_probed = 0;
        statistics.key_index_skips = 0;
        statistics.directory_owned_bytes -= self.key_index.owned_bytes();
        statistics.hits = 0;
        statistics.misses = 0;
        Self {
            directory: self.directory.clone(),
            manifest: self.manifest.clone(),
            raw_manifest: self.raw_manifest.clone(),
            limits: self.limits,
            statistics,
            key_index: KeyIndex::default(),
            frozen: self.frozen,
        }
    }
    /// Exact key lookup, newest immutable run first. A matching range is only
    /// a candidate: complete key equality in the native chunk is authoritative.
    /// Exceeding cumulative lookup work is a typed refusal, never a cache miss.
    /// This low-level reader does not silently fall back to cold reduction;
    /// callers must choose such a policy explicitly and must not hide invalid
    /// bindings, corrupted payloads or native validation errors as misses.
    pub fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        match self.lookup_outcome(key, context)? {
            SavedApplicationCacheLookupOutcome::Hit(record) => Ok(Some(record)),
            SavedApplicationCacheLookupOutcome::Absent => Ok(None),
            SavedApplicationCacheLookupOutcome::WorkBudgetExhausted => Err(AppError::limit(
                "cache cumulative lookup allowance exceeded",
            )),
        }
    }

    /// Typed optional-cache boundary. Only ordinary cumulative lookup-grant
    /// exhaustion is an outcome rather than an error. Bad keys/bindings,
    /// counter overflow, I/O, corruption and native decoding/validation still
    /// fail. Strict preparation/resume callers should use `lookup`.
    pub fn lookup_outcome(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<SavedApplicationCacheLookupOutcome, AppError> {
        key.validate(self.manifest.binding.physical_arity)
            .map_err(fail)?;
        self.lookup_fingerprinted(key, context, index::key_filter::fingerprint(key)?)
    }

    // The fingerprint can reject candidate reads only. Keeping full-key lookup
    // in this separate method also lets tests force a hash collision without
    // weakening, replacing or forging any authenticated chunk metadata.
    fn lookup_fingerprinted(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
        fingerprint: u64,
    ) -> Result<SavedApplicationCacheLookupOutcome, AppError> {
        key.validate(self.manifest.binding.physical_arity)
            .map_err(fail)?;
        let mut work = LookupWork::default();
        for (run_index, run) in self.manifest.runs.iter().enumerate().rev() {
            resource_checkpoint()?;
            self.statistics.runs_probed = self.statistics.runs_probed.saturating_add(1);
            let offset = run.partition_point(|chunk| chunk.last < *key);
            let Some(chunk) = run.get(offset).filter(|chunk| chunk.first <= *key) else {
                continue;
            };
            let slot = (run_index, offset);
            if chunk
                .key_fingerprints
                .as_ref()
                .is_some_and(|keys| !index::key_filter::may_contain(keys, fingerprint))
                || self.key_index.contains(slot, key) == Some(false)
            {
                self.statistics.key_index_skips = self.statistics.key_index_skips.saturating_add(1);
                continue;
            }
            match work.charge(chunk, self.limits) {
                Ok(true) => {}
                refused => {
                    self.statistics.lookup_limit_refusals =
                        self.statistics.lookup_limit_refusals.saturating_add(1);
                    // Counter overflow remains an error, never an optional
                    // work-budget fallback, while retaining refusal telemetry.
                    return refused
                        .map(|_| SavedApplicationCacheLookupOutcome::WorkBudgetExhausted);
                }
            }
            self.statistics.lookup_encoded_bytes_charged = self
                .statistics
                .lookup_encoded_bytes_charged
                .saturating_add(chunk.bytes.saturating_add(1));
            self.statistics.lookup_decoded_bytes_charged = self
                .statistics
                .lookup_decoded_bytes_charged
                .saturating_add(chunk.decoded_bytes.saturating_add(1));
            let bytes = index::read_bounded(
                &self.directory.join("chunks").join(chunk.file_name()),
                usize::try_from(chunk.bytes).map_err(fail)?,
            )?;
            self.statistics.chunks_read = self.statistics.chunks_read.saturating_add(1);
            self.statistics.chunk_bytes_read = self
                .statistics
                .chunk_bytes_read
                .saturating_add(bytes.len() as u64);
            self.statistics.largest_chunk_read =
                self.statistics.largest_chunk_read.max(bytes.len());
            if bytes.len() as u64 != chunk.bytes || *blake3::hash(&bytes).as_bytes() != chunk.digest
            {
                return Err(fail("cache chunk digest or length mismatch"));
            }
            let payload = compression::decode(
                &bytes,
                chunk.decoded_bytes,
                &chunk.decoded_digest,
                self.limits,
            )?;
            self.statistics.decoded_chunk_bytes = self
                .statistics
                .decoded_chunk_bytes
                .saturating_add(payload.len() as u64);
            self.statistics.largest_decoded_chunk =
                self.statistics.largest_decoded_chunk.max(payload.len());
            drop(bytes);
            let decoded =
                ApplicationCacheChunk::inspect(&payload, &self.manifest.binding, self.limits.chunk)
                    .map_err(binary_error)?;
            if decoded.len() != chunk.records
                || decoded.first_key() != Some(&chunk.first)
                || decoded.last_key() != Some(&chunk.last)
            {
                return Err(fail("cache chunk index differs from native framing"));
            }
            if let Some(expected) = &chunk.key_fingerprints {
                if index::key_filter::collect(decoded.keys())? != *expected {
                    return Err(fail(
                        "cache chunk key fingerprints differ from native framing",
                    ));
                }
            }
            let record = decoded.decode_record(key, context).map_err(binary_error)?;
            // Only a fully checked read can install an exact absence proof.
            // The slot denotes the immutable descriptor in this reader's
            // authenticated binding; a changed snapshot clears all slots.
            let directory_bytes =
                self.statistics.directory_owned_bytes - self.key_index.owned_bytes();
            if chunk.key_fingerprints.is_none() {
                self.key_index.retain(
                    slot,
                    decoded.keys(),
                    self.limits.max_directory_owned_bytes - directory_bytes,
                )?;
            }
            self.statistics.directory_owned_bytes = directory_bytes + self.key_index.owned_bytes();
            if let Some(record) = record {
                self.statistics.hits = self.statistics.hits.saturating_add(1);
                return Ok(SavedApplicationCacheLookupOutcome::Hit(record));
            }
        }
        resource_checkpoint()?;
        self.statistics.misses = self.statistics.misses.saturating_add(1);
        Ok(SavedApplicationCacheLookupOutcome::Absent)
    }

    #[cfg(test)]
    pub(super) fn lookup_outcome_with_test_fingerprint(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
        fingerprint: u64,
    ) -> Result<SavedApplicationCacheLookupOutcome, AppError> {
        self.lookup_fingerprinted(key, context, fingerprint)
    }
}

#[derive(Default)]
struct LookupWork {
    chunks: usize,
    encoded: u64,
    decoded: u64,
}
impl LookupWork {
    fn charge(
        &mut self,
        chunk: &index::Chunk,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<bool, AppError> {
        let overflow = || AppError::limit("cache lookup work counter overflow");
        let chunks = self.chunks.checked_add(1).ok_or_else(overflow)?;
        let encoded = self
            .encoded
            .checked_add(chunk.bytes)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        let decoded = self
            .decoded
            .checked_add(chunk.decoded_bytes)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        resource_checkpoint()?;
        if limits.diagnostics_enabled
            && (chunks > limits.max_lookup_chunks
                || encoded > limits.max_lookup_encoded_bytes
                || decoded > limits.max_lookup_decoded_bytes)
        {
            return Ok(false);
        }
        self.chunks = chunks;
        self.encoded = encoded;
        self.decoded = decoded;
        Ok(true)
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;

    #[test]
    fn lookup_counter_overflow_is_an_error_not_budget_exhaustion() {
        let key =
            ApplicationCacheKey::guarded_root(&rustred::family::IntegralKey::try_new([1]).unwrap());
        let chunk = index::Chunk {
            digest: [0; 32],
            bytes: 1,
            decoded_digest: [0; 32],
            decoded_bytes: 1,
            records: 1,
            first: key.clone(),
            last: key,
            key_fingerprints: None,
        };
        let limits = SavedApplicationCachePackLimits::default();
        for mut work in [
            LookupWork {
                chunks: usize::MAX,
                ..Default::default()
            },
            LookupWork {
                encoded: u64::MAX,
                ..Default::default()
            },
            LookupWork {
                decoded: u64::MAX,
                ..Default::default()
            },
        ] {
            assert_eq!(
                work.charge(&chunk, limits).unwrap_err().kind(),
                crate::AppErrorKind::Limit
            );
        }
    }
}
