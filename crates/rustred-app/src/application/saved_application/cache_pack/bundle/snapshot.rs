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
        Self::copy_selected_bases(output, bindings, bases, limits, false)
    }

    /// Copy an ordered union of captured bases into a new independent bundle.
    /// Earlier caller bases have precedence, including authoritative empty
    /// records. Every source run and shadowed reference is preserved; only the
    /// physical copy list is deduplicated by compatible compressed-chunk hash.
    /// Each output namespace starts a fresh generation at one. Source generation
    /// numbers are not comparable across independent stores.
    ///
    /// Sources with one binding may come from different directories. The chosen
    /// source of each physical chunk is length/hash verified, without decoding
    /// coefficients; compatible duplicate sources need not be read. Conflicting
    /// descriptors fail before destination creation. No source file is changed.
    ///
    /// The existing referenced-byte limits still apply, even when duplicate
    /// references cost only one physical copy. Additional coexisting directory,
    /// origin-index and encoded-index memory shares the directory allowance;
    /// caller-owned readers are not copied into that scratch. Disk admission,
    /// publication order and partial-output failure behavior match
    /// `copy_bases_to_new`. This copies payloads, not an active reduction DAG.
    pub fn copy_base_union_to_new(
        output: &Path,
        bindings: &[ApplicationCacheBinding],
        bases_newest_first: &[&SavedApplicationCachePack],
        limits: SavedApplicationCachePackLimits,
    ) -> Result<(Self, SavedApplicationCacheCopyStatistics), AppError> {
        Self::copy_selected_bases(output, bindings, bases_newest_first, limits, true)
    }

    fn copy_selected_bases(
        output: &Path,
        bindings: &[ApplicationCacheBinding],
        bases: &[&SavedApplicationCachePack],
        limits: SavedApplicationCachePackLimits,
        union: bool,
    ) -> Result<(Self, SavedApplicationCacheCopyStatistics), AppError> {
        let selection_bound = if union {
            preflight_namespace_metadata(bindings, bases.len(), limits)?
        } else {
            0
        };
        let manifest = Manifest {
            namespaces: namespaces(bindings, limits)?,
        };
        let mut bundle_limits = limits;
        if union {
            let names = shared_metadata_bytes(&manifest, 0, 0, &[])?
                .checked_add(selection_bound)
                .ok_or_else(|| fail("snapshot namespace metadata size overflow"))?;
            admit_metadata(names, limits)?;
            bundle_limits.max_manifest_bytes = limits
                .max_manifest_bytes
                .min(limits.max_directory_owned_bytes - names);
        }
        let bundle_bytes = encode(&manifest, bundle_limits)?;
        let selection_bytes = manifest
            .namespaces
            .len()
            .checked_mul(std::mem::size_of::<Vec<&SavedApplicationCachePack>>())
            .ok_or_else(|| fail("snapshot base selection size overflow"))?;
        if union {
            let prospective = shared_metadata_bytes(&manifest, bundle_bytes.capacity(), 0, &[])?
                .checked_add(selection_bytes)
                .and_then(|sum| {
                    sum.checked_add(
                        bases
                            .len()
                            .checked_mul(std::mem::size_of::<&SavedApplicationCachePack>())?,
                    )
                })
                .ok_or_else(|| fail("snapshot base selection size overflow"))?;
            admit_metadata(prospective, limits)?;
        }
        resource_allocation(selection_bytes)?;
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(manifest.namespaces.len())
            .map_err(fail)?;
        selected.resize_with(manifest.namespaces.len(), Vec::new);
        for &base in bases {
            let position = manifest
                .namespaces
                .iter()
                .position(|namespace| &namespace.binding == base.binding())
                .ok_or_else(|| fail("base pack is not in the exact snapshot namespace set"))?;
            if !union && !selected[position].is_empty() {
                return Err(fail("multiple base packs for one snapshot namespace"));
            }
            resource_allocation(std::mem::size_of::<&SavedApplicationCachePack>())?;
            selected[position].try_reserve_exact(1).map_err(fail)?;
            selected[position].push(base);
        }
        let shared_bytes = shared_metadata_bytes(
            &manifest,
            bundle_bytes.capacity(),
            selected.capacity(),
            &selected,
        )?;
        if union {
            admit_metadata(shared_bytes, limits)?;
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
        for (namespace, bases) in manifest.namespaces.iter().zip(&selected) {
            let child = prepare_child(namespace, bases, limits, union, shared_bytes)?;
            statistics.index_bytes = statistics
                .index_bytes
                .checked_add(child.bytes.len() as u64)
                .ok_or_else(|| fail("snapshot index byte count overflow"))?;
            statistics.copied_chunks = statistics
                .copied_chunks
                .checked_add(child.chunks.len())
                .ok_or_else(|| fail("snapshot chunk count overflow"))?;
            for source in &child.chunks {
                statistics.compressed_bytes = statistics
                    .compressed_bytes
                    .checked_add(source.chunk.bytes)
                    .ok_or_else(|| fail("snapshot compressed byte count overflow"))?;
            }
            statistics.preserved_record_references = statistics
                .preserved_record_references
                .checked_add(child.records)
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
        for (namespace, bases) in manifest.namespaces.iter().zip(&selected) {
            let child = prepare_child(namespace, bases, limits, union, shared_bytes)?;
            let directory = child_path(output, namespace);
            fs::create_dir(&directory).map_err(fail)?;
            fs::create_dir(directory.join("chunks")).map_err(fail)?;
            writer::sync_directory(&directory.join("chunks"))?;
            for source in &child.chunks {
                let chunk = source.chunk;
                let source = source.directory.join("chunks").join(chunk.file_name());
                copy_chunk(
                    &source,
                    &directory.join("chunks").join(chunk.file_name()),
                    chunk,
                )?;
            }
            writes.write(&directory.join(index::MANIFEST), &child.bytes, false)?;
        }
        // Child index publication fsyncs each child, then sync the directory
        // containing those child names before making the top-level index visible.
        writer::sync_directory(&output.join("bindings"))?;
        writes.write(&output.join(FILE), &bundle_bytes, false)?;
        writer::sync_directory(writer::parent_directory(output))?;
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

fn preflight_namespace_metadata(
    bindings: &[ApplicationCacheBinding],
    bases: usize,
    limits: SavedApplicationCachePackLimits,
) -> Result<usize, AppError> {
    let selection = bindings
        .len()
        .checked_mul(std::mem::size_of::<Vec<&SavedApplicationCachePack>>())
        .and_then(|sum| {
            sum.checked_add(bases.checked_mul(std::mem::size_of::<&SavedApplicationCachePack>())?)
        })
        .ok_or_else(|| fail("snapshot base selection size overflow"))?;
    // The existing namespace builder grows its vector by push. Reserve a
    // conservative double-length capacity (at least its initial four slots)
    // before calling it, plus cloned
    // strings and both possible structural encoding buffers. A binding's fixed
    // fields and standard varint prefixes fit within the 96-byte margin.
    let mut names = bindings
        .len()
        .checked_mul(2)
        .map(|count| count.max(4))
        .and_then(|count| count.checked_mul(std::mem::size_of::<Namespace>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Manifest>()))
        .and_then(|bytes| bytes.checked_add(selection))
        .ok_or_else(|| fail("snapshot namespace metadata size overflow"))?;
    let mut encoded = 64usize;
    let mut binding_scratch = 0usize;
    for binding in bindings {
        let strings = binding
            .family_fingerprint
            .len()
            .checked_add(binding.layer.len())
            .ok_or_else(|| fail("snapshot namespace string size overflow"))?;
        names = names
            .checked_add(strings)
            .ok_or_else(|| fail("snapshot namespace metadata size overflow"))?;
        let bound = strings
            .checked_add(96)
            .ok_or_else(|| fail("snapshot binding encoding size overflow"))?;
        binding_scratch = binding_scratch.max(
            bound
                .checked_mul(2)
                .ok_or_else(|| fail("snapshot binding encoding size overflow"))?,
        );
        encoded = encoded
            .checked_add(bound)
            .and_then(|sum| sum.checked_add(32))
            .ok_or_else(|| fail("snapshot namespace encoding size overflow"))?;
    }
    let encoded = encoded
        .checked_mul(2)
        .ok_or_else(|| fail("snapshot namespace encoding size overflow"))?;
    let prospective = names
        .checked_add(encoded.max(binding_scratch))
        .ok_or_else(|| fail("snapshot namespace metadata size overflow"))?;
    admit_metadata(prospective, limits)?;
    resource_allocation(prospective)?;
    Ok(selection)
}

fn shared_metadata_bytes(
    manifest: &Manifest,
    encoded_capacity: usize,
    selection_capacity: usize,
    selected: &[Vec<&SavedApplicationCachePack>],
) -> Result<usize, AppError> {
    let mut total = std::mem::size_of::<Manifest>()
        .checked_add(encoded_capacity)
        .and_then(|sum| {
            sum.checked_add(
                manifest
                    .namespaces
                    .capacity()
                    .checked_mul(std::mem::size_of::<Namespace>())?,
            )
        })
        .and_then(|sum| {
            sum.checked_add(
                selection_capacity
                    .checked_mul(std::mem::size_of::<Vec<&SavedApplicationCachePack>>())?,
            )
        })
        .ok_or_else(|| fail("snapshot shared metadata size overflow"))?;
    for namespace in &manifest.namespaces {
        total = total
            .checked_add(namespace.binding.family_fingerprint.capacity())
            .and_then(|sum| sum.checked_add(namespace.binding.layer.capacity()))
            .ok_or_else(|| fail("snapshot shared metadata size overflow"))?;
    }
    for bases in selected {
        total = total
            .checked_add(
                bases
                    .capacity()
                    .checked_mul(std::mem::size_of::<&SavedApplicationCachePack>())
                    .ok_or_else(|| fail("snapshot base selection size overflow"))?,
            )
            .ok_or_else(|| fail("snapshot shared metadata size overflow"))?;
    }
    Ok(total)
}

fn admit_metadata(bytes: usize, limits: SavedApplicationCachePackLimits) -> Result<(), AppError> {
    if bytes > limits.max_directory_owned_bytes {
        Err(AppError::limit(
            "snapshot coexisting metadata exceeds directory allowance",
        ))
    } else {
        resource_checkpoint()
    }
}

struct PreparedChild<'a> {
    bytes: Vec<u8>,
    chunks: Vec<SourceChunk<'a>>,
    records: usize,
}

#[derive(Clone, Copy)]
struct SourceChunk<'a> {
    directory: &'a Path,
    chunk: &'a index::Chunk,
    precedence: usize,
}

fn prepare_child<'a>(
    namespace: &Namespace,
    bases: &[&'a SavedApplicationCachePack],
    limits: SavedApplicationCachePackLimits,
    union: bool,
    shared_bytes: usize,
) -> Result<PreparedChild<'a>, AppError> {
    let mut runs = 0usize;
    let mut count = 0usize;
    let mut directory_bytes = std::mem::size_of::<index::Manifest>()
        .checked_add(namespace.binding.family_fingerprint.len())
        .and_then(|sum| sum.checked_add(namespace.binding.layer.len()))
        .ok_or_else(|| fail("snapshot child metadata size overflow"))?;
    for base in bases {
        let original = base.captured_snapshot().1;
        index::validate(original, &namespace.binding, limits)?;
        runs = runs
            .checked_add(original.runs.len())
            .ok_or_else(|| fail("snapshot run count overflow"))?;
        for run in &original.runs {
            count = count
                .checked_add(run.len())
                .ok_or_else(|| fail("snapshot chunk reference count overflow"))?;
            directory_bytes = directory_bytes
                .checked_add(std::mem::size_of::<Vec<index::Chunk>>())
                .and_then(|sum| {
                    sum.checked_add(run.len().checked_mul(std::mem::size_of::<index::Chunk>())?)
                })
                .ok_or_else(|| fail("snapshot child metadata size overflow"))?;
            for chunk in run {
                directory_bytes = directory_bytes
                    .checked_add(chunk.first.owned_heap_bytes())
                    .and_then(|sum| sum.checked_add(chunk.last.owned_heap_bytes()))
                    .and_then(|sum| {
                        sum.checked_add(
                            chunk.key_fingerprints.as_ref().map_or(Some(0), |keys| {
                                keys.len().checked_mul(std::mem::size_of::<u64>())
                            })?,
                        )
                    })
                    .ok_or_else(|| fail("snapshot child metadata size overflow"))?;
            }
        }
    }
    if limits.diagnostics_enabled && (runs > limits.max_runs || count > limits.max_chunks) {
        return Err(AppError::limit(
            "snapshot combined run or chunk allowance exceeded",
        ));
    }
    let origin_bytes = count
        .checked_mul(std::mem::size_of::<SourceChunk<'_>>())
        .ok_or_else(|| fail("snapshot source index size overflow"))?;
    let prospective = shared_bytes
        .checked_add(directory_bytes)
        .and_then(|sum| sum.checked_add(origin_bytes))
        .ok_or_else(|| fail("snapshot coexisting metadata size overflow"))?;
    if union {
        admit_metadata(prospective, limits)?;
    } else if count
        .checked_mul(std::mem::size_of::<&index::Chunk>())
        .ok_or_else(|| fail("snapshot chunk reference index overflow"))?
        > limits.max_directory_owned_bytes
    {
        return Err(AppError::limit(
            "snapshot chunk reference index exceeds allowance",
        ));
    }
    resource_allocation(
        directory_bytes
            .checked_add(origin_bytes)
            .ok_or_else(|| fail("snapshot allocation size overflow"))?,
    )?;
    let mut child = index::Manifest {
        binding: namespace.binding.clone(),
        bundle_managed: true,
        generation: if union {
            1
        } else {
            bases.first().map_or(1, |base| base.generation())
        },
        runs: Vec::new(),
    };
    child.runs.try_reserve_exact(runs).map_err(fail)?;
    // Runs are oldest first; reverse only base precedence, never a base's runs.
    for base in bases.iter().rev() {
        child
            .runs
            .extend(base.captured_snapshot().1.runs.iter().cloned());
    }
    let statistics = index::validate(&child, &namespace.binding, limits)?;
    let chunks = unique_chunks(bases, count)?;
    let live_bytes = shared_bytes
        .checked_add(statistics.directory_owned_bytes)
        .and_then(|sum| {
            sum.checked_add(
                chunks
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceChunk<'_>>())?,
            )
        })
        .ok_or_else(|| fail("snapshot coexisting metadata size overflow"))?;
    let mut encoding_limits = limits;
    if union {
        admit_metadata(live_bytes, limits)?;
        let remaining = limits.max_directory_owned_bytes - live_bytes;
        encoding_limits.max_manifest_bytes = limits.max_manifest_bytes.min(remaining);
    }
    let bytes = index::encode(&child, encoding_limits)?;
    let with_index = live_bytes
        .checked_add(bytes.capacity())
        .ok_or_else(|| fail("snapshot coexisting index size overflow"))?;
    if union {
        admit_metadata(with_index, limits)?;
    }
    // A newly opened reader retains both forms, plus the raw index Arc header.
    let reader_bytes = statistics
        .directory_owned_bytes
        .checked_add(bytes.len())
        .and_then(|sum| {
            sum.checked_add(std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>())
        })
        .ok_or_else(|| fail("snapshot reader metadata size overflow"))?;
    if union {
        admit_metadata(reader_bytes, limits)?;
    }
    Ok(PreparedChild {
        bytes,
        chunks,
        records: statistics.records,
    })
}

/// Chunk hashes address physical files, but immutable payload descriptor checks
/// reject collisions before deduplication. Legacy descriptors may omit the new
/// negative filter for an otherwise identical payload. Two supplied filters
/// must agree. The original run/index order is untouched.
fn unique_chunks<'a>(
    bases: &[&'a SavedApplicationCachePack],
    count: usize,
) -> Result<Vec<SourceChunk<'a>>, AppError> {
    let bytes = count
        .checked_mul(std::mem::size_of::<SourceChunk<'_>>())
        .ok_or_else(|| fail("snapshot chunk reference index overflow"))?;
    resource_checkpoint()?;
    let mut chunks = Vec::new();
    resource_allocation(bytes)?;
    chunks.try_reserve_exact(count).map_err(fail)?;
    for (precedence, base) in bases.iter().enumerate() {
        let (directory, manifest) = base.captured_snapshot();
        chunks.extend(manifest.runs.iter().flatten().map(|chunk| SourceChunk {
            directory,
            chunk,
            precedence,
        }));
    }
    // Group present filters together before legacy omissions: otherwise a
    // None between two contradictory Some filters could hide the mismatch in
    // the adjacent-descriptor check below. Prefer present metadata on dedup.
    chunks.sort_unstable_by_key(|source| {
        (
            source.chunk.digest,
            source.chunk.key_fingerprints.is_none(),
            source.precedence,
        )
    });
    if chunks.windows(2).any(|pair| {
        pair[0].chunk.digest == pair[1].chunk.digest
            && !pair[0].chunk.compatible_payload(pair[1].chunk)
    }) {
        return Err(fail("colliding snapshot chunk descriptors"));
    }
    chunks.dedup_by_key(|source| source.chunk.digest);
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
    crate::application::atomic_file::write_file_atomically_with_resources(
        destination,
        false,
        |output| {
            let mut buffer = [0u8; TRANSFER_BYTES];
            let mut remaining = chunk.bytes;
            let mut hash = blake3::Hasher::new();
            while remaining != 0 {
                resource_checkpoint()?;
                let read = usize::try_from(remaining.min(buffer.len() as u64))
                    .expect("bounded buffer size");
                source_file
                    .read_exact(&mut buffer[..read])
                    .map_err(io_error)?;
                hash.update(&buffer[..read]);
                output.write_all(&buffer[..read]).map_err(io_error)?;
                remaining -= read as u64;
            }
            if source_file.read(&mut buffer[..1]).map_err(io_error)? != 0
                || hash.finalize().as_bytes() != &chunk.digest
            {
                return Err(fail("snapshot chunk length or checksum mismatch"));
            }
            Ok(())
        },
    )
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
