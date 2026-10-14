//! Portable immutable-base copy: framing/hashes only, never coefficient decode.
use super::*;
use std::io::{Read, Write};

const TRANSFER_BYTES: usize = 64 * 1024;

/// Successful base publication only. Resident overlay completion is a separate
/// caller-owned operation. Stored records count original index references,
/// including shadowed entries; this is not a distinct-key or cache-hit count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationCacheCopyStatistics {
    pub namespaces: usize,
    pub copied_bases: usize,
    pub copied_chunks: usize,
    pub preserved_record_references: usize,
    pub compressed_bytes: u64,
    pub index_bytes: u64,
    pub transfer_buffer_bytes: usize,
}

impl SavedApplicationCacheBundleWriter {
    /// Copy captured immutable bases into a NEW, independent bundle, then
    /// return its writer for resident overlays. Bindings include the exact
    /// program/terminal-convention identity; no namespace is inferred from
    /// loop count. A namespace without a base starts empty. Duplicate or
    /// unknown bases are rejected before any filesystem mutation.
    ///
    /// Uses each reader's captured index, not a later checkpoint at its path.
    /// Every referenced compressed chunk is length/hash verified while streamed
    /// through one 64 KiB buffer. No native state, coefficients or full key index
    /// is decoded. Source files are never linked, modified or removed; separate
    /// destination inodes also work across filesystems. Trusted-generated input
    /// remains the trust boundary: byte verification does not prove identities.
    ///
    /// All destination chunks/indexes are pre-admitted under one disk quota.
    /// Metadata scratch is bounded by one child directory and a sorted chunk
    /// reference vector. Source readers' existing directory memory is not cloned
    /// across all namespaces. Old runs and shadowed records are preserved.
    ///
    /// The bundle index is installed LAST. An interrupted/failed copy retains
    /// its owned partial destination; it is not silently resumed or overwritten.
    /// A final directory-sync error may leave a complete installed index but
    /// still returns Err, so durability is not claimed. A successful return
    /// means only the base is complete: an overlay failure must remain explicit.
    /// Existing outputs always refuse, including earlier partial copies. Normal
    /// writer resume is available only after successful base publication; no
    /// preparation-job metadata or in-flight computation is copied.
    pub fn copy_bases_to_new(
        output: &Path,
        bindings: &[ApplicationCacheBinding],
        bases: &[&SavedApplicationCachePack],
        limits: SavedApplicationCachePackLimits,
    ) -> Result<(Self, SavedApplicationCacheCopyStatistics), AppError> {
        let manifest = Manifest {
            namespaces: namespaces(bindings, limits)?,
        };
        let bundle_bytes = encode(&manifest, limits)?;
        let mut selected = vec![None; manifest.namespaces.len()];
        for &base in bases {
            let position = manifest
                .namespaces
                .iter()
                .position(|namespace| &namespace.binding == base.binding())
                .ok_or_else(|| fail("base pack is not in the exact snapshot namespace set"))?;
            if selected[position].replace(base).is_some() {
                return Err(fail("multiple base packs for one snapshot namespace"));
            }
        }
        let mut statistics = SavedApplicationCacheCopyStatistics {
            namespaces: manifest.namespaces.len(),
            copied_bases: bases.len(),
            index_bytes: bundle_bytes.len() as u64,
            transfer_buffer_bytes: TRANSFER_BYTES,
            ..Default::default()
        };
        // Exact preflight before creating the output. Re-encoding bounded
        // structural metadata later avoids retaining all namespace indexes.
        for (namespace, base) in manifest.namespaces.iter().zip(&selected) {
            let child = child_manifest(namespace, *base, limits)?;
            let bytes = index::encode(&child, limits)?;
            statistics.index_bytes = statistics
                .index_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| fail("snapshot index byte count overflow"))?;
            let chunks = unique_chunks(&child, limits)?;
            statistics.copied_chunks = statistics
                .copied_chunks
                .checked_add(chunks.len())
                .ok_or_else(|| fail("snapshot chunk count overflow"))?;
            for chunk in chunks {
                statistics.compressed_bytes = statistics
                    .compressed_bytes
                    .checked_add(chunk.bytes)
                    .ok_or_else(|| fail("snapshot compressed byte count overflow"))?;
            }
            statistics.preserved_record_references = statistics
                .preserved_record_references
                .checked_add(index::validate(&child, &namespace.binding, limits)?.records)
                .ok_or_else(|| fail("snapshot record reference count overflow"))?;
        }
        let total = statistics
            .compressed_bytes
            .checked_add(statistics.index_bytes)
            .ok_or_else(|| fail("snapshot disk byte count overflow"))?;
        let quota = Arc::new(quota::Quota::new(limits.max_disk_bytes, 0)?);
        // The one atomic staging inode becomes its installed destination; this
        // reserves all bytes BEFORE either a payload or index is written.
        quota.reserve(total)?;
        fs::create_dir(output).map_err(fail)?;
        let lock = writer::lock(output)?;
        fs::create_dir(output.join("bindings")).map_err(fail)?;
        let mut writes = quota::WriteState::default();
        for (namespace, base) in manifest.namespaces.iter().zip(&selected) {
            let child = child_manifest(namespace, *base, limits)?;
            let directory = child_path(output, namespace);
            fs::create_dir(&directory).map_err(fail)?;
            fs::create_dir(directory.join("chunks")).map_err(fail)?;
            for chunk in unique_chunks(&child, limits)? {
                let source = base
                    .expect("nonempty manifest has a base")
                    .captured_snapshot()
                    .0
                    .join("chunks")
                    .join(chunk.file_name());
                copy_chunk(
                    &source,
                    &directory.join("chunks").join(chunk.file_name()),
                    chunk,
                )?;
            }
            writes.write(
                &directory.join(index::MANIFEST),
                &index::encode(&child, limits)?,
                false,
            )?;
        }
        // Child index publication fsyncs each child, then sync the directory
        // containing those child names before making the top-level index visible.
        fs::File::open(output.join("bindings"))
            .and_then(|dir| dir.sync_all())
            .map_err(fail)?;
        writes.write(&output.join(FILE), &bundle_bytes, false)?;
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(fail)?;
        Ok((
            Self {
                directory: output.to_path_buf(),
                manifest,
                limits,
                quota,
                writes,
                #[cfg(test)]
                fail_child_after_install: false,
                _lock: lock,
            },
            statistics,
        ))
    }
}

fn child_manifest(
    namespace: &Namespace,
    base: Option<&SavedApplicationCachePack>,
    limits: SavedApplicationCachePackLimits,
) -> Result<index::Manifest, AppError> {
    if let Some(base) = base {
        let original = base.captured_snapshot().1;
        index::validate(original, &namespace.binding, limits)?;
        let mut manifest = original.clone();
        manifest.bundle_managed = true;
        Ok(manifest)
    } else {
        Ok(index::Manifest {
            binding: namespace.binding.clone(),
            bundle_managed: true,
            generation: 1,
            runs: Vec::new(),
        })
    }
}

/// Chunk hashes address physical files, but immutable payload descriptor checks
/// reject collisions before deduplication. Legacy descriptors may omit the new
/// negative filter for an otherwise identical payload. Two supplied filters
/// must agree. The original run/index order is untouched.
fn unique_chunks(
    manifest: &index::Manifest,
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<&index::Chunk>, AppError> {
    let count = manifest
        .runs
        .iter()
        .try_fold(0usize, |count, run| count.checked_add(run.len()))
        .ok_or_else(|| fail("snapshot chunk reference count overflow"))?;
    let bytes = count
        .checked_mul(std::mem::size_of::<&index::Chunk>())
        .ok_or_else(|| fail("snapshot chunk reference index overflow"))?;
    if count > limits.max_chunks || bytes > limits.max_directory_owned_bytes {
        return Err(AppError::limit(
            "snapshot chunk reference index exceeds allowance",
        ));
    }
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(count).map_err(fail)?;
    chunks.extend(manifest.runs.iter().flatten());
    // Group present filters together before legacy omissions: otherwise a
    // None between two contradictory Some filters could hide the mismatch in
    // the adjacent-descriptor check below. Prefer present metadata on dedup.
    chunks.sort_unstable_by_key(|chunk| (chunk.digest, chunk.key_fingerprints.is_none()));
    if chunks
        .windows(2)
        .any(|pair| pair[0].digest == pair[1].digest && !pair[0].compatible_payload(pair[1]))
    {
        return Err(fail("colliding snapshot chunk descriptors"));
    }
    chunks.dedup_by_key(|chunk| chunk.digest);
    Ok(chunks)
}

fn copy_chunk(source: &Path, destination: &Path, chunk: &index::Chunk) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(source).map_err(fail)?;
    if !metadata.is_file() || metadata.len() != chunk.bytes {
        return Err(fail("snapshot source is not the declared regular chunk"));
    }
    let mut source_file = fs::File::open(source).map_err(fail)?;
    let opened = source_file.metadata().map_err(fail)?;
    if !opened.is_file() || opened.len() != chunk.bytes {
        return Err(fail("snapshot source changed before copy"));
    }
    crate::application::atomic_file::write_file_atomically_with(destination, false, |output| {
        let mut buffer = [0u8; TRANSFER_BYTES];
        let mut remaining = chunk.bytes;
        let mut hash = blake3::Hasher::new();
        while remaining != 0 {
            let read =
                usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded buffer size");
            source_file
                .read_exact(&mut buffer[..read])
                .map_err(|error| error.to_string())?;
            hash.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
            remaining -= read as u64;
        }
        if source_file
            .read(&mut buffer[..1])
            .map_err(|error| error.to_string())?
            != 0
            || hash.finalize().as_bytes() != &chunk.digest
        {
            return Err("snapshot chunk length or checksum mismatch".into());
        }
        Ok(())
    })
    .map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_transfer_spans_multiple_buffers_without_decoding_and_refuses_growth_or_truncation()
    {
        let scratch = crate::application::saved_application::tests::Scratch::new();
        let source = scratch.0.join("source-bytes");
        let data = (0..TRANSFER_BYTES * 3 + 7)
            .map(|n| (n % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&source, &data).unwrap();
        let key = rustred::persistence::application_cache::ApplicationCacheKey::guarded_root(
            &rustred::family::IntegralKey::try_new([1]).unwrap(),
        );
        let chunk = index::Chunk {
            digest: *blake3::hash(&data).as_bytes(),
            bytes: data.len() as u64,
            decoded_digest: [0; 32],
            decoded_bytes: 1,
            records: 1,
            first: key.clone(),
            last: key,
            key_fingerprints: None,
        };
        let output = scratch.0.join("copied-bytes");
        copy_chunk(&source, &output, &chunk).unwrap();
        assert_eq!(fs::read(output).unwrap(), data);
        let mut corrupt = data.clone();
        corrupt[TRANSFER_BYTES + 2] ^= 1;
        fs::write(&source, corrupt).unwrap();
        let failed = scratch.0.join("wrong-hash");
        assert!(copy_chunk(&source, &failed, &chunk).is_err());
        assert!(!failed.exists());
        fs::write(&source, &data[..data.len() - 1]).unwrap();
        assert!(copy_chunk(&source, &scratch.0.join("truncated"), &chunk).is_err());
        fs::write(&source, [&data[..], &[0]].concat()).unwrap();
        assert!(copy_chunk(&source, &scratch.0.join("grown"), &chunk).is_err());
    }
}
