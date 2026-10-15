use super::*;
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub(crate) struct CommitOutcome {
    pub generation: ValidatedGeneration,
    /// Optional cleanup is after the durable commit, never a false rollback.
    pub cleanup_warning: Option<String>,
}

/// One leased writer. This low-level generated-state API validates structure and
/// monotonicity, not the native mathematical closure claim. The future backend
/// must supply only sealed native deltas; no core baseline import is exposed.
pub(crate) struct StoreWriter {
    directory: PathBuf,
    execution: RawExecutionBinding,
    limits: StorageLimits,
    current: Option<ValidatedGeneration>,
    charged: u64,
    files: usize,
    poisoned: bool,
    _lock: File,
    #[cfg(test)]
    fail_at: Option<Stage>,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    AfterPages,
    AfterState,
    BeforeCurrent,
    AfterCurrent,
    CurrentUncertain,
    CancelAfterPages,
}

impl StoreWriter {
    pub fn create(
        directory: &Path,
        execution: RawExecutionBinding,
        limits: StorageLimits,
    ) -> Result<Self> {
        wire::binding(&execution, limits)?;
        fs::create_dir(directory).map_err(io)?;
        for name in ["pages", "states", "generations"] {
            fs::create_dir(directory.join(name)).map_err(io)?;
        }
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(io)?;
        let parent = directory
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(io)?;
        Self::open_writer(directory, execution, limits, true)
    }
    pub fn resume_generated(
        directory: &Path,
        execution: RawExecutionBinding,
        limits: StorageLimits,
    ) -> Result<Self> {
        Self::open_writer(directory, execution, limits, false)
    }
    fn open_writer(
        directory: &Path,
        execution: RawExecutionBinding,
        limits: StorageLimits,
        fresh: bool,
    ) -> Result<Self> {
        wire::binding(&execution, limits)?;
        reader::check_directories(directory)?;
        let lock_path = directory.join("writer.lock");
        if lock_path.is_symlink() {
            return Err(corrupt("finite writer lock is a symlink"));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(io)?;
        if !lock.metadata().map_err(io)?.is_file() {
            return Err(corrupt("finite writer lock is not regular"));
        }
        lock.try_lock()
            .map_err(|error| StorageError::new(ErrorKind::Locked, error))?;
        let (charged, files) = scan(directory, limits)?;
        let current = match fs::symlink_metadata(directory.join(wire::CURRENT)) {
            Ok(_) if !fresh => Some(ValidatedGeneration::open_generated(
                directory, &execution, limits,
            )?),
            Ok(_) => {
                return Err(StorageError::new(
                    ErrorKind::Stale,
                    "CURRENT appeared inside a newly created store",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && fresh => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(corrupt(
                    "CURRENT is missing; ordinary resume cannot infer that no generation was committed",
                ));
            }
            Err(error) => return Err(io(error)),
        };
        Ok(Self {
            directory: directory.to_path_buf(),
            execution,
            limits,
            current,
            charged,
            files,
            poisoned: false,
            _lock: lock,
            #[cfg(test)]
            fail_at: None,
        })
    }
    pub fn current(&self) -> Result<Option<&ValidatedGeneration>> {
        self.healthy()?;
        Ok(self.current.as_ref())
    }
    pub fn charged_disk_bytes(&self) -> u64 {
        self.charged
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Persist one structurally validated generated-state transition. The
    /// caller must stop tracing if this fails: the proposed cursor is not
    /// authoritative until a successful outcome (or a trusted reopen resolves
    /// an indeterminate installation). No files are garbage-collected here.
    pub fn commit_generated(
        &mut self,
        mut delta: Vec<StoredWork>,
        state: GenerationState,
        cancel: &AtomicBool,
    ) -> Result<CommitOutcome> {
        self.healthy()?;
        check(cancel)?;
        self.check_current()?;
        let arity = wire::binding(&self.execution, self.limits)?;
        if delta.len() > self.limits.max_delta_records
            || wire::work_payload(&delta, delta.capacity())? > self.limits.max_delta_payload_bytes
        {
            return Err(resource("finite delta sort-buffer allowance"));
        }
        for key in &delta {
            check(cancel)?;
            key.validate(arity)?;
        }
        delta.sort_unstable();
        if delta.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(corrupt("duplicate Work in completed delta"));
        }
        let previous_work = self
            .current
            .as_ref()
            .map_or(0, |old| old.state.completed_work);
        let records = previous_work
            .checked_add(u64::try_from(delta.len()).map_err(resource)?)
            .ok_or_else(|| resource("cumulative Work overflow"))?;
        wire::validate_state(&state, &self.execution, records, self.limits)?;
        if let Some(old) = &self.current {
            wire::validate_extension(&old.state, &state)?;
            let reader = old.reader(self.limits)?;
            for key in &delta {
                check(cancel)?;
                if reader.lookup(key)? == Lookup::Present {
                    return Err(corrupt("completed delta repeats earlier Work"));
                }
            }
        }
        let generation = self.current.as_ref().map_or(Ok(1), |old| {
            old.generation()
                .checked_add(1)
                .ok_or_else(|| resource("finite generation overflow"))
        })?;
        let mut runs = self
            .current
            .as_ref()
            .map_or_else(Vec::new, |old| old.manifest.runs.clone());
        let page_count = delta.len().div_ceil(self.limits.max_page_records);
        let old_pages = runs
            .iter()
            .try_fold(0usize, |sum, run| sum.checked_add(run.len()))
            .ok_or_else(|| resource("finite page count overflow"))?;
        if (!delta.is_empty() && runs.len() >= self.limits.max_runs)
            || old_pages
                .checked_add(page_count)
                .is_none_or(|n| n > self.limits.max_pages)
        {
            return Err(resource("finite immutable-run fan-out allowance"));
        }
        let mut run = Vec::new();
        run.try_reserve_exact(page_count).map_err(resource)?;
        // Only this bounded delta is sorted/materialized; older Work is neither
        // decoded nor rewritten. Future fan-out/compaction still needs a gate.
        let mut incoming = delta.into_iter();
        loop {
            let page_records: Vec<_> = incoming
                .by_ref()
                .take(self.limits.max_page_records)
                .collect();
            if page_records.is_empty() {
                break;
            }
            check(cancel)?;
            let first = page_records.first().unwrap().clone();
            let last = page_records.last().unwrap().clone();
            let count = page_records.len() as u64;
            let bytes = wire::encode(
                wire::PAGE_MAGIC,
                &wire::Page {
                    execution: self.execution.clone(),
                    records: page_records,
                },
                self.limits.max_page_bytes,
            )?;
            let object = self.install("pages", "rrwork", &bytes)?;
            run.push(wire::PageRef {
                object,
                records: count,
                first,
                last,
            });
        }
        if !run.is_empty() {
            runs.push(run);
        }
        #[cfg(test)]
        self.fail_before(Stage::AfterPages)?;
        #[cfg(test)]
        if self
            .fail_at
            .take_if(|stage| *stage == Stage::CancelAfterPages)
            .is_some()
        {
            cancel.store(true, Ordering::Release);
        }
        check(cancel)?;
        let scope_digest = wire::scope_digest(&state.scope, self.limits.max_state_bytes)?;
        // This bounded first slice rewrites cumulative state and the page
        // directory each generation. State I/O is O(history metadata), not a
        // demonstrated full-R6 persistence/compaction strategy.
        let state_file = wire::StateFile {
            execution: self.execution.clone(),
            generation,
            state,
        };
        let state_bytes =
            wire::encode(wire::STATE_MAGIC, &state_file, self.limits.max_state_bytes)?;
        let state_ref = self.install("states", "rrstate", &state_bytes)?;
        drop(state_bytes);
        #[cfg(test)]
        self.fail_before(Stage::AfterState)?;
        check(cancel)?;
        let manifest = wire::Manifest {
            execution: self.execution.clone(),
            generation,
            previous: self.current.as_ref().map(|old| old.digest),
            state: state_ref,
            scope_digest,
            runs,
        };
        wire::validate_manifest(&manifest, &self.execution, self.limits)?;
        let manifest_bytes = wire::encode(
            wire::MANIFEST_MAGIC,
            &manifest,
            self.limits.max_manifest_bytes,
        )?;
        let reference = self.install("generations", "rrgen", &manifest_bytes)?;
        drop(manifest_bytes);
        #[cfg(test)]
        self.fail_before(Stage::BeforeCurrent)?;
        check(cancel)?;
        self.check_current()?;
        let tip = wire::Tip {
            generation,
            manifest: reference.clone(),
        };
        let tip_bytes = wire::encode(wire::TIP_MAGIC, &tip, wire::TIP_LIMIT)?;
        let old_tip_bytes = if self.current.is_some() {
            fs::symlink_metadata(self.directory.join(wire::CURRENT))
                .map_err(io)?
                .len()
        } else {
            0
        };
        self.reserve(
            tip_bytes.len() as u64,
            usize::from(self.current.is_none()),
            usize::from(self.current.is_some()),
        )?;
        self.poisoned = true;
        let result = crate::application::atomic_file::write_file_atomically(
            &self.directory.join(wire::CURRENT),
            &tip_bytes,
            true,
        );
        if let Err(error) = result {
            return Err(StorageError::new(
                ErrorKind::Indeterminate,
                format!(
                    "CURRENT installation/sync uncertain; drop and reopen before continuation: {error}"
                ),
            ));
        }
        #[cfg(test)]
        if self
            .fail_at
            .take_if(|stage| *stage == Stage::CurrentUncertain)
            .is_some()
        {
            return Err(StorageError::new(
                ErrorKind::Indeterminate,
                "injected post-install uncertainty; reopen required",
            ));
        }
        self.charged = self
            .charged
            .checked_sub(old_tip_bytes)
            .ok_or_else(|| corrupt("finite disk accounting underflow"))?;
        let validated = ValidatedGeneration {
            directory: self.directory.clone(),
            digest: reference.digest,
            manifest: Arc::new(manifest),
            state: Arc::new(state_file.state),
        };
        self.current = Some(validated.clone());
        self.poisoned = false;
        let cleanup_warning = None;
        #[cfg(test)]
        let cleanup_warning = if self
            .fail_at
            .take_if(|stage| *stage == Stage::AfterCurrent)
            .is_some()
        {
            Some("injected optional cleanup failure after durable CURRENT".to_owned())
        } else {
            cleanup_warning
        };
        Ok(CommitOutcome {
            generation: validated,
            cleanup_warning,
        })
    }
    fn healthy(&self) -> Result<()> {
        if self.poisoned {
            Err(StorageError::new(
                ErrorKind::Indeterminate,
                "finite writer requires drop/reopen/rescan",
            ))
        } else {
            Ok(())
        }
    }
    fn check_current(&mut self) -> Result<()> {
        let result = (|| match (
            &self.current,
            fs::symlink_metadata(self.directory.join(wire::CURRENT)),
        ) {
            (None, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            (Some(expected), Ok(_)) => {
                let bytes = wire::read(&self.directory.join(wire::CURRENT), wire::TIP_LIMIT)?;
                let tip: wire::Tip = wire::decode(wire::TIP_MAGIC, &bytes, wire::TIP_LIMIT)?;
                if tip.generation == expected.generation() && tip.manifest.digest == expected.digest
                {
                    Ok(())
                } else {
                    Err(StorageError::new(
                        ErrorKind::Stale,
                        "finite CURRENT changed since writer acquisition",
                    ))
                }
            }
            _ => Err(StorageError::new(
                ErrorKind::Stale,
                "finite CURRENT appeared/disappeared",
            )),
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn reserve(&mut self, bytes: u64, files: usize, transient_names: usize) -> Result<()> {
        let used = self
            .charged
            .checked_add(bytes)
            .ok_or_else(|| resource("physical finite disk overflow"))?;
        let count = self
            .files
            .checked_add(files)
            .ok_or_else(|| resource("physical finite file overflow"))?;
        let peak_names = count
            .checked_add(transient_names)
            .ok_or_else(|| resource("transient finite file overflow"))?;
        if used > self.limits.max_disk_bytes || peak_names > self.limits.max_physical_files {
            return Err(resource("physical finite-store disk/file allowance"));
        }
        self.charged = used;
        self.files = count;
        Ok(())
    }
    fn install(&mut self, subdir: &str, suffix: &str, bytes: &[u8]) -> Result<wire::ObjectRef> {
        self.healthy()?;
        let reference = wire::ObjectRef {
            digest: *blake3::hash(bytes).as_bytes(),
            bytes: bytes.len() as u64,
        };
        let path = self
            .directory
            .join(subdir)
            .join(wire::name(&reference.digest, suffix));
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                if wire::read(&path, bytes.len())? != bytes {
                    return Err(corrupt("existing immutable finite object differs"));
                }
                return Ok(reference);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io(error)),
        }
        // create-if-absent briefly retains both staging and installed hard-link
        // pathnames. Reserve that metadata slot before writing any bytes.
        self.reserve(bytes.len() as u64, 1, 1)?;
        self.poisoned = true;
        crate::application::atomic_file::write_file_atomically(&path, bytes, false).map_err(io)?;
        self.poisoned = false;
        Ok(reference)
    }
    #[cfg(test)]
    fn fail_before(&mut self, stage: Stage) -> Result<()> {
        if self.fail_at.take_if(|value| *value == stage).is_some() {
            self.poisoned = true;
            Err(io(format!("injected stop at {stage:?}")))
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    pub(super) fn inject_failure(&mut self, stage: Stage) {
        self.fail_at = Some(stage);
    }
}

fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(StorageError::new(
            ErrorKind::Cancelled,
            "finite commit cancelled before CURRENT",
        ))
    } else {
        Ok(())
    }
}
fn scan(directory: &Path, limits: StorageLimits) -> Result<(u64, usize)> {
    let mut bytes = 0u64;
    let mut files = 0usize;
    for subdir in [
        directory.to_path_buf(),
        directory.join("pages"),
        directory.join("states"),
        directory.join("generations"),
    ] {
        for entry in fs::read_dir(&subdir).map_err(io)? {
            let entry = entry.map_err(io)?;
            let kind = entry.file_type().map_err(io)?;
            if subdir == directory && kind.is_dir() {
                if ["pages", "states", "generations"]
                    .iter()
                    .any(|name| entry.file_name() == *name)
                {
                    continue;
                }
                return Err(corrupt("foreign directory inside finite store"));
            }
            if !kind.is_file() {
                return Err(corrupt("finite store contains a nonregular file"));
            }
            files = files
                .checked_add(1)
                .ok_or_else(|| resource("physical file count overflow"))?;
            bytes = bytes
                .checked_add(entry.metadata().map_err(io)?.len())
                .ok_or_else(|| resource("physical disk overflow"))?;
            if files > limits.max_physical_files || bytes > limits.max_disk_bytes {
                return Err(resource("resumed finite-store physical quota"));
            }
        }
    }
    Ok((bytes, files))
}
