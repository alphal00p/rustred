use rustred::algebra::CoefficientContext;
use rustred::persistence::application_cache::{
    ApplicationCacheBinding, ApplicationCacheRecord, encode_application_cache_chunk,
};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};

use super::*;
use crate::AppError;

/// Single writer, bounded chunk construction, atomic resumable publication.
/// Completed records can be supplied before their RAM cache entries are
/// evicted. Neither this writer nor its checkpoint stores an active DAG.
pub struct SavedApplicationCacheWriter {
    directory: PathBuf,
    manifest: index::Manifest,
    limits: SavedApplicationCachePackLimits,
    physical_chunk_bytes: u64,
    physical_chunks: usize,
    quota: Arc<quota::Quota>,
    writes: quota::WriteState,
    // Advisory OS lock is released even when the owning process exits. It is
    // not a create_new sentinel that can strand a resumed preparation run.
    _lock: File,
}

impl SavedApplicationCacheWriter {
    pub fn create(
        directory: &Path,
        binding: ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        Self::create_with_quota(
            directory,
            binding,
            limits,
            Arc::new(quota::Quota::new(limits.max_disk_bytes, 0)?),
            false,
        )
    }
    pub(super) fn create_with_quota(
        directory: &Path,
        binding: ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
        quota: Arc<quota::Quota>,
        bundle_managed: bool,
    ) -> Result<Self, AppError> {
        binding.validate(limits.chunk).map_err(fail)?;
        fs::create_dir(directory).map_err(fail)?;
        fs::create_dir(directory.join("chunks")).map_err(fail)?;
        let lock = lock(directory)?;
        let mut writer = Self {
            directory: directory.to_path_buf(),
            manifest: index::Manifest {
                binding,
                bundle_managed,
                generation: 0,
                runs: Vec::new(),
            },
            limits,
            physical_chunk_bytes: 0,
            physical_chunks: 0,
            quota,
            writes: Default::default(),
            _lock: lock,
        };
        writer.checkpoint()?;
        Ok(writer)
    }
    pub fn resume(
        directory: &Path,
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        Self::resume_with_quota(directory, expected, limits, None)
    }
    pub(super) fn resume_with_quota(
        directory: &Path,
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCachePackLimits,
        quota: Option<Arc<quota::Quota>>,
    ) -> Result<Self, AppError> {
        let lock = lock(directory)?;
        let bytes =
            index::read_bounded(&directory.join(index::MANIFEST), limits.max_manifest_bytes)?;
        let (manifest, _) = index::decode(&bytes, expected, limits)?;
        if manifest.bundle_managed != quota.is_some() {
            return Err(fail(
                "bundle-managed store requires its aggregate-quota writer",
            ));
        }
        // Count interrupted/unreferenced files too; ignoring their content must
        // not allow a resumed writer to evade the requested disk ceiling.
        let (physical_chunk_bytes, physical_chunks, manifest_bytes) =
            quota::scan_store(directory, limits)?;
        let quota = match quota {
            Some(quota) => quota,
            None => Arc::new(quota::Quota::new(
                limits.max_disk_bytes,
                physical_chunk_bytes
                    .checked_add(manifest_bytes)
                    .ok_or_else(|| fail("cache disk count overflow"))?,
            )?),
        };
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest,
            limits,
            physical_chunk_bytes,
            physical_chunks,
            quota,
            writes: Default::default(),
            _lock: lock,
        })
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.manifest.binding
    }
    pub fn statistics(&self) -> Result<SavedApplicationCachePackStatistics, AppError> {
        index::validate(&self.manifest, &self.manifest.binding, self.limits)
    }
    pub fn physical_chunk_bytes(&self) -> u64 {
        self.physical_chunk_bytes
    }
    /// Includes sibling stores when this writer belongs to a bundle, and any
    /// conservative reservation retained after a failed atomic write.
    pub fn charged_disk_bytes(&self) -> Result<u64, AppError> {
        self.quota.used()
    }
    pub fn is_poisoned(&self) -> bool {
        self.writes.is_poisoned()
    }
    #[cfg(test)]
    pub(super) fn inject_post_install_failure(&mut self) {
        self.writes.inject_post_install_failure();
    }

    /// One bounded sorted batch becomes one immutable compressed chunk. A
    /// disjoint range extends the newest run in sorted order; overlapping ranges
    /// start a new run, preserving newest-key precedence. Calls retain neither
    /// input nor encoded payload. A checkpoint publishes the updated directory.
    /// Construction temporarily owns native and compressed buffers plus a
    /// bounded prospective directory copy, not an unbounded per-entry index.
    pub fn append_completed(
        &mut self,
        records: &[ApplicationCacheRecord],
        context: &CoefficientContext,
    ) -> Result<(), AppError> {
        self.writes.ensure_open()?;
        if records.is_empty() {
            return Ok(());
        }
        let decoded = encode_application_cache_chunk(
            &self.manifest.binding,
            records,
            context,
            self.limits.chunk,
        )
        .map_err(fail)?;
        let bytes = compression::encode(&decoded, self.limits)?;
        let chunk = index::Chunk {
            digest: *blake3::hash(&bytes).as_bytes(),
            bytes: bytes.len() as u64,
            decoded_digest: *blake3::hash(&decoded).as_bytes(),
            decoded_bytes: decoded.len() as u64,
            records: records.len(),
            first: records.first().expect("nonempty").key.clone(),
            last: records.last().expect("nonempty").key.clone(),
            key_fingerprints: Some(index::key_filter::collect(
                records.iter().map(|record| &record.key),
            )?),
        };
        drop(decoded);
        // No file is created until all prospective range/directory bounds pass.
        // This is metadata only, bounded independently of coefficient payloads.
        let mut prospective = self.manifest.clone();
        let coalesced = if let Some(run) = prospective.runs.last_mut() {
            let offset = run.partition_point(|existing| existing.last < chunk.first);
            if run.get(offset).is_none_or(|next| chunk.last < next.first) {
                run.insert(offset, chunk.clone());
                true
            } else {
                false
            }
        } else {
            false
        };
        if !coalesced {
            prospective.runs.push(vec![chunk.clone()]);
        }
        index::validate(&prospective, &prospective.binding, self.limits)?;
        let path = self.directory.join("chunks").join(chunk.file_name());
        if path.exists() {
            let existing = index::read_bounded(&path, self.limits.max_encoded_chunk_bytes)?;
            if existing != bytes {
                return Err(fail("content-addressed cache chunk collision"));
            }
        } else {
            let total = self
                .physical_chunk_bytes
                .checked_add(chunk.bytes)
                .ok_or_else(|| fail("cache disk byte overflow"))?;
            if self.physical_chunks >= self.limits.max_chunks || total > self.limits.max_disk_bytes
            {
                return Err(AppError::limit("cache disk or chunk allowance reached"));
            }
            self.quota.reserve(chunk.bytes)?;
            self.writes.write(&path, &bytes, false)?;
            self.physical_chunk_bytes = total;
            self.physical_chunks += 1;
        }
        self.manifest = prospective;
        Ok(())
    }
    pub fn checkpoint(&mut self) -> Result<(), AppError> {
        self.writes.ensure_open()?;
        let previous = self.manifest.generation;
        self.manifest.generation = previous
            .checked_add(1)
            .ok_or_else(|| fail("cache checkpoint generation overflow"))?;
        let result = (|| {
            let bytes = index::encode(&self.manifest, self.limits)?;
            if self.physical_chunk_bytes.saturating_add(bytes.len() as u64)
                > self.limits.max_disk_bytes
            {
                return Err(AppError::limit("cache checkpoint exceeds disk allowance"));
            }
            let path = self.directory.join(index::MANIFEST);
            let previous_bytes = match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => metadata.len(),
                Ok(_) => return Err(fail("cache checkpoint is not a regular file")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
                Err(error) => return Err(fail(error)),
            };
            self.quota.reserve(bytes.len() as u64)?;
            self.writes.write(&path, &bytes, true)?;
            if let Err(error) = self.quota.release_replaced(previous_bytes) {
                self.writes.poison();
                return Err(error);
            }
            Ok(())
        })();
        if result.is_err() {
            self.manifest.generation = previous;
        }
        result
    }
}

pub(super) fn lock(directory: &Path) -> Result<File, AppError> {
    let path = directory.join("writer.lock");
    if path.is_symlink() {
        return Err(fail("cache writer lock is a symbolic link"));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(fail)?;
    file.try_lock().map_err(fail)?;
    Ok(file)
}
