//! A generic bundle is an exact set of child program bindings, not a loop-count
//! dispatch table. Child writes share one quota before touching disk.
mod snapshot;
pub use snapshot::SavedApplicationCacheCopyStatistics;

use super::*;
use crate::{AppError, application::atomic_file::write_file_atomically_resource};
use rustred::algebra::CoefficientContext;
use rustred::persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheRecord};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

const FILE: &str = "bundle.rrindex";
const MAGIC: &[u8; 16] = b"RR-CACHE-BNDL-02";
const METADATA: &str = "preparation.rrcheckpoint";
const METADATA_MAGIC: &[u8; 16] = b"RR-CACHE-JOB-001";

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct Namespace {
    id: [u8; 32],
    binding: ApplicationCacheBinding,
}
#[derive(bincode::Encode, bincode::Decode)]
struct Manifest {
    namespaces: Vec<Namespace>,
}

fn namespaces(
    bindings: &[ApplicationCacheBinding],
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<Namespace>, AppError> {
    resource_checkpoint()?;
    if bindings.is_empty() || (limits.diagnostics_enabled && bindings.len() > limits.max_namespaces)
    {
        return Err(fail("invalid cache bundle namespace count"));
    }
    let mut namespaces = Vec::new();
    for binding in bindings {
        binding.validate(limits.chunk).map_err(binary_error)?;
        let mut bytes = index::BoundedBytes::new(limits.max_manifest_bytes);
        bincode::encode_into_std_write(binding, &mut bytes, bincode::config::standard())
            .map_err(encode_error)?;
        namespaces.push(Namespace {
            id: *blake3::hash(&bytes.finish()).as_bytes(),
            binding: binding.clone(),
        });
    }
    namespaces.sort_by_key(|namespace| namespace.id);
    if namespaces.windows(2).any(|pair| pair[0].id == pair[1].id) {
        // Even equal bindings must not appear twice. A digest collision never
        // aliases distinct programs or merges their coefficient contexts.
        return Err(fail("duplicate or colliding cache bundle namespace"));
    }
    Ok(namespaces)
}

fn encode(
    manifest: &Manifest,
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<u8>, AppError> {
    resource_checkpoint()?;
    let mut out = index::BoundedBytes::new(limits.max_manifest_bytes.saturating_sub(32));
    out.write_all(MAGIC).map_err(io_error)?;
    bincode::encode_into_std_write(manifest, &mut out, bincode::config::standard())
        .map_err(encode_error)?;
    let mut bytes = out.finish();
    let digest = blake3::hash(&bytes);
    resource_allocation(32usize.saturating_sub(bytes.capacity() - bytes.len()))?;
    bytes.try_reserve_exact(32).map_err(fail)?;
    bytes.extend_from_slice(digest.as_bytes());
    Ok(bytes)
}
fn read(
    directory: &Path,
    bindings: &[ApplicationCacheBinding],
    limits: SavedApplicationCachePackLimits,
) -> Result<(Manifest, usize), AppError> {
    resource_checkpoint()?;
    let expected = namespaces(bindings, limits)?;
    let bytes = index::read_bounded(&directory.join(FILE), limits.max_manifest_bytes)?;
    if bytes.len() < MAGIC.len() + 32 || !bytes.starts_with(MAGIC) {
        return Err(fail("invalid cache bundle framing"));
    }
    let end = bytes.len() - 32;
    if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(fail("cache bundle digest mismatch"));
    }
    // Namespace identities are already known. Compare their exact wire
    // encoding before allocating any supplied directory counts.
    let manifest = Manifest {
        namespaces: expected,
    };
    let expected_bytes = encode(&manifest, limits)?;
    if bytes != expected_bytes {
        return Err(fail("cache bundle program bindings differ"));
    }
    Ok((manifest, bytes.len()))
}
fn child_path(directory: &Path, namespace: &Namespace) -> PathBuf {
    directory
        .join("bindings")
        .join(blake3::Hash::from_bytes(namespace.id).to_hex().as_str())
}
fn find<'a>(
    manifest: &'a Manifest,
    binding: &ApplicationCacheBinding,
) -> Result<&'a Namespace, AppError> {
    manifest
        .namespaces
        .iter()
        .find(|namespace| namespace.binding == *binding)
        .ok_or_else(|| fail("program is not bound by this cache bundle"))
}

/// Holds only the bounded namespace directory. Child range indexes are opened
/// explicitly, so an aggregate four-program bundle never eagerly loads all
/// coefficient data or hides an unbounded collection of child readers.
pub struct SavedApplicationCacheBundle {
    directory: PathBuf,
    manifest: Manifest,
    limits: SavedApplicationCachePackLimits,
}
impl SavedApplicationCacheBundle {
    pub fn open_generated(
        directory: &Path,
        bindings: &[ApplicationCacheBinding],
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        let (manifest, _) = read(directory, bindings, limits)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest,
            limits,
        })
    }
    pub fn bindings(&self) -> impl Iterator<Item = &ApplicationCacheBinding> {
        self.manifest
            .namespaces
            .iter()
            .map(|namespace| &namespace.binding)
    }
    pub fn open_reader(
        &self,
        binding: &ApplicationCacheBinding,
    ) -> Result<SavedApplicationCachePack, AppError> {
        let namespace = find(&self.manifest, binding)?;
        SavedApplicationCachePack::open_generated(
            &child_path(&self.directory, namespace),
            binding,
            self.limits,
        )
    }
    /// Bounded binary job metadata only. Its caller must validate its workload
    /// digest/options; this sidecar has no coefficient or closure authority.
    pub fn metadata(&self) -> Result<Option<Vec<u8>>, AppError> {
        let path = self.directory.join(METADATA);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(fail(error)),
            Ok(_) => {}
        }
        let mut bytes = index::read_bounded(&path, self.limits.max_manifest_bytes)?;
        if bytes.len() < METADATA_MAGIC.len() + 32 || !bytes.starts_with(METADATA_MAGIC) {
            return Err(fail("invalid preparation metadata framing"));
        }
        let end = bytes.len() - 32;
        if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
            return Err(fail("preparation metadata digest mismatch"));
        }
        bytes.truncate(end);
        bytes.drain(..METADATA_MAGIC.len());
        Ok(Some(bytes))
    }
}

pub struct SavedApplicationCacheBundleWriter {
    directory: PathBuf,
    manifest: Manifest,
    limits: SavedApplicationCachePackLimits,
    quota: Arc<quota::Quota>,
    writes: quota::WriteState,
    #[cfg(test)]
    fail_child_after_install: bool,
    _lock: fs::File,
}
impl SavedApplicationCacheBundleWriter {
    pub fn create(
        directory: &Path,
        bindings: &[ApplicationCacheBinding],
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        let manifest = Manifest {
            namespaces: namespaces(bindings, limits)?,
        };
        let bytes = encode(&manifest, limits)?;
        let quota = Arc::new(quota::Quota::new(limits.max_disk_bytes, 0)?);
        fs::create_dir(directory).map_err(fail)?;
        fs::create_dir(directory.join("bindings")).map_err(fail)?;
        let lock = writer::lock(directory)?;
        for namespace in &manifest.namespaces {
            SavedApplicationCacheWriter::create_with_quota(
                &child_path(directory, namespace),
                namespace.binding.clone(),
                limits,
                quota.clone(),
                true,
            )?;
        }
        quota.reserve(bytes.len() as u64)?;
        write_file_atomically_resource(&directory.join(FILE), &bytes, false)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest,
            limits,
            quota,
            writes: Default::default(),
            #[cfg(test)]
            fail_child_after_install: false,
            _lock: lock,
        })
    }
    pub fn resume(
        directory: &Path,
        bindings: &[ApplicationCacheBinding],
        limits: SavedApplicationCachePackLimits,
    ) -> Result<Self, AppError> {
        let lock = writer::lock(directory)?;
        let (manifest, bytes) = read(directory, bindings, limits)?;
        let metadata_bytes = metadata_bytes(&directory.join(METADATA))?;
        let staging_bytes = quota::staging_bytes(directory, &[FILE, METADATA], limits)?;
        let mut used = (bytes as u64)
            .checked_add(metadata_bytes)
            .and_then(|sum| sum.checked_add(staging_bytes))
            .ok_or_else(|| fail("bundle staging count overflow"))?;
        for namespace in &manifest.namespaces {
            let path = child_path(directory, namespace);
            let (chunks, _, index_bytes) = quota::scan_store(&path, limits)?;
            used = used
                .checked_add(chunks)
                .and_then(|sum| sum.checked_add(index_bytes))
                .ok_or_else(|| fail("bundle disk count overflow"))?;
            // Verify every child identity, but drop its range index immediately.
            SavedApplicationCachePack::open_generated(&path, &namespace.binding, limits)?;
        }
        let quota = Arc::new(quota::Quota::new(limits.max_disk_bytes, used)?);
        Ok(Self {
            directory: directory.to_path_buf(),
            manifest,
            limits,
            quota,
            writes: Default::default(),
            #[cfg(test)]
            fail_child_after_install: false,
            _lock: lock,
        })
    }
    /// Publish one complete bounded batch to its exact program namespace.
    /// All sibling chunks/indexes and temporary replacement index bytes share
    /// the quota. A failed write leaves previous checkpoints readable.
    pub fn append_completed(
        &mut self,
        binding: &ApplicationCacheBinding,
        records: &[ApplicationCacheRecord],
        context: &CoefficientContext,
    ) -> Result<(), AppError> {
        self.writes.ensure_open()?;
        let namespace = find(&self.manifest, binding)?;
        let mut child = SavedApplicationCacheWriter::resume_with_quota(
            &child_path(&self.directory, namespace),
            binding,
            self.limits,
            Some(self.quota.clone()),
        )?;
        #[cfg(test)]
        if std::mem::take(&mut self.fail_child_after_install) {
            child.inject_post_install_failure();
        }
        let result = child
            .append_completed(records, context)
            .and_then(|()| child.checkpoint());
        if child.is_poisoned() {
            self.writes.poison();
        }
        result
    }
    pub fn charged_disk_bytes(&self) -> Result<u64, AppError> {
        self.quota.used()
    }
    pub fn is_poisoned(&self) -> bool {
        self.writes.is_poisoned()
    }
    /// Atomically publish caller-encoded binary progress through the SAME
    /// aggregate quota, including temporary replacement bytes. No arbitrary
    /// sidecar path is accepted and interrupted staging remains charged.
    pub fn checkpoint_metadata(&mut self, metadata: &[u8]) -> Result<(), AppError> {
        self.writes.ensure_open()?;
        let mut out = index::BoundedBytes::new(self.limits.max_manifest_bytes.saturating_sub(32));
        out.write_all(METADATA_MAGIC).map_err(io_error)?;
        out.write_all(metadata).map_err(io_error)?;
        let mut bytes = out.finish();
        let digest = blake3::hash(&bytes);
        resource_allocation(32usize.saturating_sub(bytes.capacity() - bytes.len()))?;
        bytes.try_reserve_exact(32).map_err(fail)?;
        bytes.extend_from_slice(digest.as_bytes());
        let path = self.directory.join(METADATA);
        let previous = metadata_bytes(&path)?;
        self.quota.reserve(bytes.len() as u64)?;
        self.writes.write(&path, &bytes, true)?;
        if let Err(error) = self.quota.release_replaced(previous) {
            self.writes.poison();
            return Err(error);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn inject_child_post_install_failure(&mut self) {
        self.fail_child_after_install = true;
    }
}

fn metadata_bytes(path: &Path) -> Result<u64, AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(metadata.len()),
        Ok(_) => Err(fail("preparation metadata is not a regular file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(fail(error)),
    }
}
