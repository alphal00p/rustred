use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::application::{AppError, atomic_file::write_file_atomically};

use super::super::CandidateBundleLimits;
use super::{CandidateCheckpointOptions, CheckpointManifest};

const MANIFEST: &str = "checkpoint.toml";
const LOCK: &str = "checkpoint.lock";

/// Small structural/durability receipt. Counts do not establish rule validity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::application::candidate_bundle) struct CheckpointReceipt {
    pub ordinal: usize,
    pub bytes: usize,
    pub rules: usize,
    pub finite_residuals: usize,
}

#[derive(Debug)]
enum Slot {
    Missing,
    /// Also retained after an ambiguous publication error. It is deliberately
    /// not reusable in this session: the destination may already be installed.
    Reserved,
    Committed(CheckpointReceipt),
}

#[derive(Debug)]
struct DiskState {
    charged_bytes: usize,
    slots: Vec<Slot>,
}

/// One owner for the entire solve/assembly session; safe concurrent publication
/// to distinct ordinals. The OS lock is held by this non-cloned File until drop.
/// Never unlink the lockfile: another owner could otherwise lock a new inode.
#[derive(Debug)]
pub(in crate::application::candidate_bundle) struct CheckpointStore {
    directory: PathBuf,
    manifest: CheckpointManifest,
    limits: CandidateBundleLimits,
    max_total_bytes: usize,
    _lock: File,
    state: Mutex<DiskState>,
}

impl CheckpointStore {
    pub(in crate::application::candidate_bundle) fn open(
        options: &CandidateCheckpointOptions,
        manifest: CheckpointManifest,
        limits: CandidateBundleLimits,
    ) -> Result<Self, AppError> {
        manifest.validate(limits)?;
        if options.max_total_bytes == 0 || options.directory.as_os_str().is_empty() {
            return Err(AppError::input(
                "checkpoint requires a directory and positive byte budget",
            ));
        }
        if !options.resume {
            // Do not create a tree of parent directories as an incidental side
            // effect. The caller supplies an existing parent and owns cleanup.
            match fs::create_dir(&options.directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error("create directory", &options.directory, error)),
            }
        }
        let directory_meta = fs::symlink_metadata(&options.directory)
            .map_err(|error| io_error("inspect directory", &options.directory, error))?;
        if !directory_meta.file_type().is_dir() {
            return Err(AppError::input(
                "checkpoint path must be a directory, not a symlink",
            ));
        }
        let lock_path = options.directory.join(LOCK);
        if let Ok(meta) = fs::symlink_metadata(&lock_path) {
            if !meta.file_type().is_file() {
                return Err(AppError::input("checkpoint lock must be a regular file"));
            }
        }
        // File::try_lock is stable in Rust 1.89 (the app's MSRV). Both access
        // modes also support the Windows implementation; never truncate it.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| io_error("open lock", &lock_path, error))?;
        lock.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => {
                AppError::execution("checkpoint directory already has an active owner")
            }
            TryLockError::Error(error) => io_error("lock directory", &lock_path, error),
        })?;
        let mut state = DiskState {
            charged_bytes: 0,
            slots: (0..manifest.sectors().len())
                .map(|_| Slot::Missing)
                .collect(),
        };
        let mut found_manifest = false;
        let mut installed = Vec::new();
        let max_files = manifest
            .sectors()
            .len()
            .checked_add(limits.max_collection_entries)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| AppError::limit("checkpoint file count overflow"))?;
        for (count, entry) in fs::read_dir(&options.directory)
            .map_err(|error| io_error("list directory", &options.directory, error))?
            .enumerate()
        {
            if count >= max_files {
                return Err(AppError::limit(
                    "checkpoint directory exceeds its file-count limit",
                ));
            }
            let entry = entry
                .map_err(|error| io_error("read directory entry", &options.directory, error))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)
                .map_err(|error| io_error("inspect file", &path, error))?;
            if !meta.file_type().is_file() {
                return Err(AppError::input(
                    "checkpoint entries must be regular files, not symlinks/directories",
                ));
            }
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| AppError::input("checkpoint has an unrecognized file name"))?;
            if !options.resume && name != LOCK {
                return Err(AppError::input(
                    "new checkpoint directory must be empty; use explicit resume for an existing campaign",
                ));
            }
            let bytes = usize::try_from(meta.len())
                .map_err(|_| AppError::limit("checkpoint file length overflows usize"))?;
            charge(&mut state.charged_bytes, bytes, options.max_total_bytes)?;
            match name {
                LOCK => {}
                MANIFEST => found_manifest = true,
                _ => {
                    if let Some(ordinal) = sector_ordinal(name, manifest.sectors().len()) {
                        installed.push((ordinal, bytes));
                    } else if !is_staging(name, manifest.sectors().len()) {
                        return Err(AppError::input(format!(
                            "unexpected checkpoint file {name:?}"
                        )));
                    }
                    // Recognized abandoned staging files are not completions,
                    // but remain charged. We do not delete any previous work.
                }
            }
        }
        let manifest_path = options.directory.join(MANIFEST);
        let manifest_limit = limits.bundle_byte_limit().min(options.max_total_bytes);
        if options.resume {
            if !found_manifest {
                return Err(AppError::input(
                    "resume requires an installed checkpoint manifest",
                ));
            }
            let bytes = read_bounded(&manifest_path, manifest_limit)?;
            manifest.admit_manifest(&bytes, limits)?;
            installed.sort_unstable_by_key(|(ordinal, _)| *ordinal);
            // Manifest/request admission precedes even structural shard reads.
            // Native payloads are imported only during final assembly.
            for (ordinal, expected_bytes) in installed {
                let bytes = read_bounded(
                    &options.directory.join(sector_name(ordinal)),
                    limits.bundle_byte_limit(),
                )?;
                if bytes.len() != expected_bytes {
                    return Err(AppError::execution(
                        "checkpoint shard changed while opening the session",
                    ));
                }
                let (rules, finite_residuals) = manifest.admit_shard(ordinal, &bytes, limits)?;
                state.slots[ordinal] = Slot::Committed(CheckpointReceipt {
                    ordinal,
                    bytes: bytes.len(),
                    rules,
                    finite_residuals,
                });
            }
        } else {
            let bytes = manifest.encode(manifest_limit)?;
            charge(
                &mut state.charged_bytes,
                bytes.len(),
                options.max_total_bytes,
            )?;
            write_file_atomically(&manifest_path, &bytes, false).map_err(AppError::execution)?;
        }
        Ok(Self {
            directory: options.directory.clone(),
            manifest,
            limits,
            max_total_bytes: options.max_total_bytes,
            _lock: lock,
            state: Mutex::new(state),
        })
    }

    /// Original manifest ordinals, not the filtered executor's local ordinals.
    /// Call before scheduling workers; in-progress/failed publication is an
    /// error rather than a new opportunity to solve or overwrite the same slot.
    pub(in crate::application::candidate_bundle) fn pending(
        &self,
    ) -> Result<Vec<(usize, Vec<bool>)>, AppError> {
        let state = self.state()?;
        if state
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Reserved))
        {
            return Err(AppError::execution(
                "checkpoint has an in-progress or failed publication",
            ));
        }
        Ok(state
            .slots
            .iter()
            .enumerate()
            .filter_map(|(ordinal, slot)| {
                matches!(slot, Slot::Missing)
                    .then(|| (ordinal, self.manifest.sectors()[ordinal].clone()))
            })
            .collect())
    }

    /// Returns installed structural receipts in original manifest order.
    pub(in crate::application::candidate_bundle) fn receipts(
        &self,
    ) -> Result<Vec<CheckpointReceipt>, AppError> {
        Ok(self
            .state()?
            .slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Committed(receipt) => Some(receipt.clone()),
                _ => None,
            })
            .collect())
    }

    pub(in crate::application::candidate_bundle) fn charged_bytes(
        &self,
    ) -> Result<usize, AppError> {
        Ok(self.state()?.charged_bytes)
    }

    /// Input is an ordinary native Candidates bundle containing exactly one
    /// completed sector. No native algebra is imported on this write path.
    pub(in crate::application::candidate_bundle) fn publish(
        &self,
        ordinal: usize,
        bytes: &[u8],
    ) -> Result<CheckpointReceipt, AppError> {
        self.publish_with(ordinal, bytes, |path, bytes| {
            write_file_atomically(path, bytes, false)
        })
    }

    fn publish_with(
        &self,
        ordinal: usize,
        bytes: &[u8],
        write: impl FnOnce(&Path, &[u8]) -> Result<(), String>,
    ) -> Result<CheckpointReceipt, AppError> {
        let (rules, finite_residuals) = self.manifest.admit_shard(ordinal, bytes, self.limits)?;
        {
            let mut state = self.state()?;
            if !matches!(state.slots[ordinal], Slot::Missing) {
                return Err(AppError::input(
                    "checkpoint sector is already committed or reserved",
                ));
            }
            charge(&mut state.charged_bytes, bytes.len(), self.max_total_bytes)?;
            state.slots[ordinal] = Slot::Reserved;
        }
        // Pending reservation remains charged on ANY write error, including a
        // failure after hard-link installation but before directory fsync. Only
        // a new, locked resume scan may decide whether the file was installed.
        // Staging and installed names normally share the same inode briefly;
        // the payload reservation accounts for that one write. A later scan
        // conservatively counts both names if cleanup left an extra hard link.
        write(&self.directory.join(sector_name(ordinal)), bytes).map_err(AppError::execution)?;
        let receipt = CheckpointReceipt {
            ordinal,
            bytes: bytes.len(),
            rules,
            finite_residuals,
        };
        self.state()?.slots[ordinal] = Slot::Committed(receipt.clone());
        Ok(receipt)
    }

    /// Bounded structural recheck before the caller's single native assembly
    /// import. Only committed slots can be read; incomplete output never escapes
    /// as a successfully assembled candidate bundle.
    pub(in crate::application::candidate_bundle) fn read(
        &self,
        ordinal: usize,
    ) -> Result<Vec<u8>, AppError> {
        let expected = match self.state()?.slots.get(ordinal) {
            Some(Slot::Committed(receipt)) => receipt.clone(),
            _ => {
                return Err(AppError::input(
                    "checkpoint sector has no committed receipt",
                ));
            }
        };
        let bytes = read_bounded(
            &self.directory.join(sector_name(ordinal)),
            self.limits.bundle_byte_limit().min(expected.bytes),
        )?;
        let (rules, finite_residuals) = self.manifest.admit_shard(ordinal, &bytes, self.limits)?;
        if bytes.len() != expected.bytes
            || rules != expected.rules
            || finite_residuals != expected.finite_residuals
        {
            return Err(AppError::execution(
                "checkpoint shard changed after its admission",
            ));
        }
        Ok(bytes)
    }

    fn state(&self) -> Result<MutexGuard<'_, DiskState>, AppError> {
        self.state
            .lock()
            .map_err(|_| AppError::execution("checkpoint accounting lock is poisoned"))
    }
}

fn charge(total: &mut usize, bytes: usize, limit: usize) -> Result<(), AppError> {
    let requested = total
        .checked_add(bytes)
        .ok_or_else(|| AppError::limit("checkpoint byte accounting overflow"))?;
    if requested > limit {
        return Err(AppError::limit(format!(
            "checkpoint payload/reservation bytes {requested} exceed total limit {limit}"
        )));
    }
    *total = requested;
    Ok(())
}

fn sector_name(ordinal: usize) -> String {
    format!("sector-{ordinal}.rrbin")
}

fn sector_ordinal(name: &str, count: usize) -> Option<usize> {
    let ordinal = name
        .strip_prefix("sector-")?
        .strip_suffix(".rrbin")?
        .parse::<usize>()
        .ok()?;
    (ordinal < count && sector_name(ordinal) == name).then_some(ordinal)
}

fn is_staging(name: &str, count: usize) -> bool {
    let Some((base, suffix)) = name
        .strip_prefix('.')
        .and_then(|name| name.rsplit_once(".rustred-tmp-"))
    else {
        return false;
    };
    if base != MANIFEST && sector_ordinal(base, count).is_none() {
        return false;
    }
    let Some((pid, attempt)) = suffix.split_once('-') else {
        return false;
    };
    [pid, attempt]
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
}

fn io_error(action: &str, path: &Path, error: io::Error) -> AppError {
    AppError::execution(format!("cannot {action} {}: {error}", path.display()))
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, AppError> {
    let meta =
        fs::symlink_metadata(path).map_err(|error| io_error("inspect checkpoint", path, error))?;
    if !meta.file_type().is_file() {
        return Err(AppError::input(
            "checkpoint input must be a regular file, not a symlink",
        ));
    }
    if meta.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(AppError::limit("checkpoint input exceeds its byte limit"));
    }
    let mut file = File::open(path).map_err(|error| io_error("open checkpoint", path, error))?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|error| io_error("read checkpoint", path, error))?;
        if n == 0 {
            return Ok(bytes);
        }
        let requested = bytes
            .len()
            .checked_add(n)
            .filter(|&n| n <= limit)
            .ok_or_else(|| AppError::limit("checkpoint input exceeds its byte limit"))?;
        bytes.try_reserve(n).map_err(|_| {
            AppError::limit(format!("cannot reserve {requested} checkpoint input bytes"))
        })?;
        bytes.extend_from_slice(&buffer[..n]);
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
