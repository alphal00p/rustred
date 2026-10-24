//! Shared aggregate reservation across independently bound child stores.
//! Failed writes keep a conservative charge until resume rescans disk; they
//! never release a reservation for a file that may already have been installed.
use super::*;
use crate::AppError;
use std::{fs, path::Path, sync::Mutex};

pub(super) struct Quota {
    limit: u64,
    used: Mutex<u64>,
}
impl Quota {
    pub fn new(limit: u64, used: u64) -> Result<Self, AppError> {
        if used > limit {
            return Err(AppError::limit("aggregate cache disk allowance exceeded"));
        }
        Ok(Self {
            limit,
            used: Mutex::new(used),
        })
    }
    pub fn reserve(&self, bytes: u64) -> Result<(), AppError> {
        let mut used = self.used.lock().map_err(fail)?;
        let total = used
            .checked_add(bytes)
            .ok_or_else(|| fail("aggregate cache disk count overflow"))?;
        if total > self.limit {
            return Err(AppError::limit("aggregate cache disk allowance reached"));
        }
        *used = total;
        Ok(())
    }
    pub fn release_replaced(&self, bytes: u64) -> Result<(), AppError> {
        let mut used = self.used.lock().map_err(fail)?;
        *used = used
            .checked_sub(bytes)
            .ok_or_else(|| fail("cache disk accounting underflow"))?;
        Ok(())
    }
    pub fn used(&self) -> Result<u64, AppError> {
        Ok(*self.used.lock().map_err(fail)?)
    }
}

pub(super) fn scan_store(
    directory: &Path,
    limits: SavedApplicationCachePackLimits,
) -> Result<(u64, usize, u64), AppError> {
    let mut chunk_bytes = 0u64;
    let mut chunks = 0usize;
    for entry in fs::read_dir(directory.join("chunks")).map_err(fail)? {
        let entry = entry.map_err(fail)?;
        if !entry.file_type().map_err(fail)?.is_file() {
            return Err(fail("cache chunk directory contains a nonregular file"));
        }
        chunks = chunks
            .checked_add(1)
            .ok_or_else(|| fail("physical chunk count overflow"))?;
        chunk_bytes = chunk_bytes
            .checked_add(entry.metadata().map_err(fail)?.len())
            .ok_or_else(|| fail("physical cache bytes overflow"))?;
        resource_checkpoint()?;
        if (limits.diagnostics_enabled && chunks > limits.max_chunks)
            || chunk_bytes > limits.max_disk_bytes
        {
            return Err(AppError::limit("resumed cache disk allowance exceeded"));
        }
    }
    let manifest_bytes = fs::symlink_metadata(directory.join(index::MANIFEST))
        .map_err(fail)?
        .len();
    let overhead = manifest_bytes
        .checked_add(staging_bytes(directory, &[index::MANIFEST], limits)?)
        .ok_or_else(|| fail("cache index/staging count overflow"))?;
    Ok((chunk_bytes, chunks, overhead))
}

/// Count only this writer's fixed-name atomic staging files. Never delete them
/// or touch foreign files. Chunk-directory staging was already counted above.
pub(super) fn staging_bytes(
    directory: &Path,
    names: &[&str],
    limits: SavedApplicationCachePackLimits,
) -> Result<u64, AppError> {
    let prefixes = names
        .iter()
        .map(|name| format!(".{name}.rustred-tmp-"))
        .collect::<Vec<_>>();
    let mut bytes = 0u64;
    let mut count = 0usize;
    for entry in fs::read_dir(directory).map_err(fail)? {
        let entry = entry.map_err(fail)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !prefixes.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        if !entry.file_type().map_err(fail)?.is_file() {
            return Err(fail("owned cache staging path is not a regular file"));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| fail("cache staging count overflow"))?;
        bytes = bytes
            .checked_add(entry.metadata().map_err(fail)?.len())
            .ok_or_else(|| fail("cache staging byte overflow"))?;
        resource_checkpoint()?;
        if (limits.diagnostics_enabled && count > limits.max_chunks)
            || bytes > limits.max_disk_bytes
        {
            return Err(AppError::limit("cache staging allowance exceeded"));
        }
    }
    Ok(bytes)
}

/// An atomic helper can report an error after installing its destination.
/// Such a writer cannot safely infer current physical counts until rescanned.
#[derive(Default)]
pub(super) struct WriteState {
    poisoned: bool,
    #[cfg(test)]
    fail_after_install: bool,
}
impl WriteState {
    pub fn ensure_open(&self) -> Result<(), AppError> {
        if self.poisoned {
            Err(fail(
                "uncertain cache write requires drop and resume/rescan",
            ))
        } else {
            Ok(())
        }
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    pub fn poison(&mut self) {
        self.poisoned = true;
    }
    pub fn write(&mut self, path: &Path, bytes: &[u8], force: bool) -> Result<(), AppError> {
        self.ensure_open()?;
        self.poisoned = true;
        crate::application::atomic_file::write_file_atomically_resource(path, bytes, force)?;
        #[cfg(test)]
        if std::mem::take(&mut self.fail_after_install) {
            return Err(fail("injected post-install directory-sync failure"));
        }
        self.poisoned = false;
        Ok(())
    }
    #[cfg(test)]
    pub fn inject_post_install_failure(&mut self) {
        self.fail_after_install = true;
    }
}
