//! Raw-only publication provenance. Work descriptors remain commitments to
//! OMITTED bytes; this type cannot provide a Work reader, baseline or cursor
//! resume token. Capturing it does not attest required-query completeness.
use super::*;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const CURRENT: &str = "RAW_CURRENT";
const MAGIC: &[u8] = b"RUSTRED-FINITE-RAW-CAPSULE-1\0";

#[derive(Clone, Debug)]
pub(crate) struct RawProvenance {
    directory: PathBuf,
    digest: Digest,
    manifest: Arc<wire::Manifest>,
    state: Arc<GenerationState>,
}
impl RawProvenance {
    /// Cheap synthetic resource fixture: repeated references preserve the exact
    /// raw union and scope while exercising large native merge directories.
    /// This does not simulate or certify thousands of native work batches.
    #[cfg(test)]
    pub(crate) fn with_repeated_raw_runs_for_test(&self, runs: usize) -> Self {
        let mut manifest = (*self.manifest).clone();
        let mut raw = (*manifest.raw).clone();
        for entries in [&mut raw.terminals, &mut raw.zeros] {
            if !entries.is_empty() {
                assert!(runs >= entries.len());
                let original = entries.clone();
                *entries = (0..runs)
                    .map(|index| original[index % original.len()].clone())
                    .collect();
            }
        }
        manifest.raw = Arc::new(raw);
        let bytes = wire::encode(wire::MANIFEST_MAGIC, &manifest, wire::MAX_FRAME).unwrap();
        let digest = *blake3::hash(&bytes).as_bytes();
        fs::write(
            self.directory
                .join("generations")
                .join(wire::name(&digest, "rrgen")),
            bytes,
        )
        .unwrap();
        Self {
            directory: self.directory.clone(),
            digest,
            manifest: Arc::new(manifest),
            state: Arc::clone(&self.state),
        }
    }

    /// Metadata capture only. Upstream must independently require exhausted
    /// authenticated scope and a successful full source-classified raw stream.
    pub fn capture(generation: &ValidatedGeneration) -> Self {
        Self {
            directory: generation.directory.clone(),
            digest: generation.digest,
            manifest: Arc::clone(&generation.manifest),
            state: Arc::clone(&generation.state),
        }
    }
    pub fn execution(&self) -> &RawExecutionBinding {
        &self.manifest.execution
    }
    pub fn state(&self) -> &GenerationState {
        &self.state
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
    pub fn generation(&self) -> u64 {
        self.manifest.generation
    }
    pub fn raw_census_directory(&self) -> &RawCensusDirectory {
        &self.manifest.raw
    }
    /// Every raw page is validated/classified on decode, with exact union and
    /// cross-kind checks. Opening a capsule alone performs lazy integrity only.
    pub fn visit_raw_census(
        &self,
        limits: RawCensusLimits,
        cancel: &AtomicBool,
        classify: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
        visitor: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<RawCensusSummary> {
        raw::RawView {
            directory: &self.directory,
            execution: self.execution(),
            raw: &self.manifest.raw,
        }
        .visit_raw_census(limits, cancel, classify, visitor)
    }
    /// Open a deliberately distinct raw-only namespace. The original captured
    /// generation and execution must be supplied by the publication binding,
    /// not accepted as arbitrary allocation/authority metadata from the file.
    pub fn open_capsule(
        directory: &Path,
        expected: &RawExecutionBinding,
        expected_generation: Digest,
        limits: StorageLimits,
    ) -> Result<Self> {
        wire::binding(expected, limits)?;
        for path in [
            directory.to_path_buf(),
            directory.join("generations"),
            directory.join("states"),
            directory.join("raw"),
        ] {
            if !fs::symlink_metadata(path).map_err(io)?.is_dir() {
                return Err(corrupt("raw capsule directory is nonregular or symlink"));
            }
        }
        let bytes = wire::read(&directory.join(CURRENT), wire::TIP_LIMIT)?;
        let tip: wire::Tip = wire::decode(MAGIC, &bytes, wire::TIP_LIMIT)?;
        drop(bytes);
        if tip.manifest.digest != expected_generation {
            return Err(corrupt("raw capsule captured generation differs"));
        }
        let (manifest, state) = reader::read_metadata(directory, &tip, expected, limits)?;
        reader::check_raw_files(directory, &manifest)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            digest: expected_generation,
            manifest: Arc::new(manifest),
            state: Arc::new(state),
        })
    }
    /// Fresh destination only; exact original manifest/state bytes and typed
    /// raw pages are copied and synced before RAW_CURRENT appears. No Work
    /// bytes/history are copied, and no source CURRENT is followed. A failure
    /// leaves an unreferenced partial directory for inspection, never silently
    /// deletes it or returns a successor capability.
    pub fn copy_capsule(
        &self,
        destination: &Path,
        limits: StorageLimits,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        self.copy_with(destination, limits, cancel, || Ok(()))
    }
    #[cfg(test)]
    pub(super) fn copy_with_before_marker(
        &self,
        destination: &Path,
        limits: StorageLimits,
        cancel: &AtomicBool,
        hook: impl FnOnce() -> Result<()>,
    ) -> Result<Self> {
        self.copy_with(destination, limits, cancel, hook)
    }
    fn copy_with(
        &self,
        destination: &Path,
        limits: StorageLimits,
        cancel: &AtomicBool,
        before_marker: impl FnOnce() -> Result<()>,
    ) -> Result<Self> {
        check(cancel)?;
        let (records, _) = wire::validate_manifest(&self.manifest, self.execution(), limits)?;
        wire::validate_state(&self.state, self.execution(), records, limits)?;
        let manifest_path = self
            .directory
            .join("generations")
            .join(wire::name(&self.digest, "rrgen"));
        let manifest_bytes = wire::read(&manifest_path, limits.max_manifest_bytes)?;
        if blake3::hash(&manifest_bytes).as_bytes() != &self.digest {
            return Err(corrupt("captured manifest changed before raw copy"));
        }
        let manifest = wire::ObjectRef {
            digest: self.digest,
            bytes: manifest_bytes.len() as u64,
        };
        let tip = wire::Tip {
            generation: self.generation(),
            manifest: manifest.clone(),
        };
        let marker = wire::encode(MAGIC, &tip, wire::TIP_LIMIT)?;
        let _: wire::Tip = wire::decode(MAGIC, &marker, wire::TIP_LIMIT)?;
        let mut unique = BTreeSet::new();
        let mut bytes = manifest
            .bytes
            .checked_add(self.manifest.state.bytes)
            .and_then(|n| n.checked_add(marker.len() as u64))
            .ok_or_else(|| resource("raw capsule byte overflow"))?;
        for page in self.manifest.raw.pages() {
            check(cancel)?;
            if unique.insert(page.object.digest) {
                bytes = bytes
                    .checked_add(page.object.bytes)
                    .ok_or_else(|| resource("raw capsule byte overflow"))?;
            }
        }
        let files = unique
            .len()
            .checked_add(3)
            .ok_or_else(|| resource("raw capsule file count overflow"))?;
        // One staging name coexists briefly with each installed immutable file.
        if bytes > limits.max_disk_bytes
            || files
                .checked_add(1)
                .is_none_or(|n| n > limits.max_physical_files)
        {
            return Err(resource("raw capsule disk/file allowance"));
        }
        check(cancel)?;
        fs::create_dir(destination).map_err(io)?;
        for name in ["generations", "states", "raw"] {
            fs::create_dir(destination.join(name)).map_err(io)?;
        }
        sync(destination)?;
        if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) {
            sync(parent)?;
        }
        install(
            destination,
            "generations",
            "rrgen",
            &manifest,
            &manifest_bytes,
        )?;
        drop(manifest_bytes);
        check(cancel)?;
        let state = wire::read_object(
            &self.directory,
            "states",
            "rrstate",
            &self.manifest.state,
            limits.max_state_bytes,
        )?;
        install(
            destination,
            "states",
            "rrstate",
            &self.manifest.state,
            &state,
        )?;
        drop(state);
        unique.clear();
        for page in self.manifest.raw.pages() {
            check(cancel)?;
            if !unique.insert(page.object.digest) {
                continue;
            }
            let bytes = wire::read_object(
                &self.directory,
                "raw",
                "rrraw",
                &page.object,
                limits.raw.max_page_bytes,
            )?;
            // Same bounded native page validator, no alternate parser or
            // materialization of the entire cumulative zero/terminal set.
            let decoded = raw::decode_page_bytes(&bytes, page, self.execution(), limits.raw)?;
            drop(decoded);
            install(destination, "raw", "rrraw", &page.object, &bytes)?;
        }
        before_marker()?;
        check(cancel)?;
        crate::application::atomic_file::write_file_atomically(
            &destination.join(CURRENT),
            &marker,
            false,
        )
        .map_err(|error| {
            StorageError::new(
                ErrorKind::Indeterminate,
                format!("raw capsule marker installation uncertain: {error}"),
            )
        })?;
        // Successful atomic installation includes sync. Returning a path-bound
        // Arc clone avoids decoding the same immutable metadata a second time.
        Ok(Self {
            directory: destination.to_path_buf(),
            digest: self.digest,
            manifest: Arc::clone(&self.manifest),
            state: Arc::clone(&self.state),
        })
    }
}
fn install(
    destination: &Path,
    subdir: &str,
    suffix: &str,
    reference: &wire::ObjectRef,
    bytes: &[u8],
) -> Result<()> {
    if bytes.len() as u64 != reference.bytes || blake3::hash(bytes).as_bytes() != &reference.digest
    {
        return Err(corrupt("raw capsule object identity mismatch"));
    }
    crate::application::atomic_file::write_file_atomically(
        &destination
            .join(subdir)
            .join(wire::name(&reference.digest, suffix)),
        bytes,
        false,
    )
    .map_err(io)
}
fn sync(directory: &Path) -> Result<()> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(io)
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(StorageError::new(
            ErrorKind::Cancelled,
            "raw capsule copy cancelled before marker",
        ))
    } else {
        Ok(())
    }
}
