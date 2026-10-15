use super::*;
use std::{fs, path::Path, sync::Mutex};
mod cache;
use cache::PageCache;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Present,
    Absent,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReaderStatistics {
    /// Convenience `WorkReader::lookup` calls only. Native generated tracing
    /// uses core's directory directly; page IO/cache counters below cover both.
    pub lookups: u64,
    pub present: u64,
    pub absent: u64,
    pub page_reads: u64,
    pub page_bytes_read: u64,
    pub page_cache_hits: u64,
    pub evictions: u64,
    pub uncached_large_pages: u64,
    pub retained_pages: usize,
    pub retained_page_payload_bytes: usize,
    /// Reserved slot/index capacity payload, also charged to the existing
    /// cache byte allowance. Excludes hash control bytes and allocator slack.
    pub retained_cache_metadata_bytes: usize,
}
struct CachedPage {
    digest: Digest,
    page: ValidatedWorkPage,
    bytes: usize,
}
struct ReaderState {
    cache: PageCache,
    statistics: ReaderStatistics,
}

/// A lookup holds this reader's mutex through a bounded page read/decode. This
/// serializes backing I/O (not algebra) and bounds in-flight transient pages.
/// A zero resident cache allowance is supported: every hit is read exactly.
pub(crate) struct WorkReader {
    generation: ValidatedGeneration,
    directory: TrustedGeneratedWorkDirectory,
    limits: StorageLimits,
    state: Mutex<ReaderState>,
}
impl WorkReader {
    pub(super) fn new(generation: ValidatedGeneration, limits: StorageLimits) -> Result<Self> {
        let (records, _) =
            wire::validate_manifest(&generation.manifest, generation.execution(), limits)?;
        wire::validate_state(&generation.state, generation.execution(), records, limits)?;
        let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
            generation.execution().clone(),
            generation.digest,
            generation.manifest.runs.clone(),
            limits.directory_limits(),
        )?;
        let cache = PageCache::new(
            limits.max_cached_pages.min(directory.statistics().pages),
            limits.max_cached_page_bytes,
            &generation.manifest.runs,
        )?;
        let statistics = ReaderStatistics {
            retained_cache_metadata_bytes: cache.metadata_bytes(),
            ..Default::default()
        };
        Ok(Self {
            generation,
            directory,
            limits,
            state: Mutex::new(ReaderState { cache, statistics }),
        })
    }
    pub fn generation(&self) -> Digest {
        self.generation.digest
    }
    pub(crate) fn directory(&self) -> &TrustedGeneratedWorkDirectory {
        &self.directory
    }
    pub(crate) fn snapshot(&self) -> &ValidatedGeneration {
        &self.generation
    }
    pub fn statistics(&self) -> Result<ReaderStatistics> {
        Ok(self
            .state
            .lock()
            .map_err(|_| io("finite reader state poisoned"))?
            .statistics
            .clone())
    }
    pub fn clear_pages(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io("finite reader state poisoned"))?;
        state.cache.clear();
        state.statistics.retained_pages = 0;
        state.statistics.retained_page_payload_bytes = 0;
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn assert_cache_consistent(&self) {
        self.state.lock().unwrap().cache.assert_consistent();
    }
    pub fn lookup(&self, key: &StoredWork) -> Result<Lookup> {
        key.validate(self.generation.execution().physical_arity as usize)?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io("finite reader state poisoned"))?;
            increment(&mut state.statistics.lookups, 1)?;
        }
        // Core owns all range selection and exact membership, including the
        // exhaustive negative. The provider below supplies pages, never hits.
        let outcome = self.directory.lookup(key, self)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io("finite reader state poisoned"))?;
        if outcome.present {
            increment(&mut state.statistics.present, 1)?;
            Ok(Lookup::Present)
        } else {
            increment(&mut state.statistics.absent, 1)?;
            Ok(Lookup::Absent)
        }
    }
}
impl GeneratedPageProvider for WorkReader {
    fn visit_validated_page(
        &self,
        expected: &wire::PageRef,
        remaining_read_bytes: u64,
        visitor: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> std::result::Result<GeneratedPageReadUsage, GeneratedWorkError> {
        self.visit_page(expected, remaining_read_bytes, visitor)
            .map_err(Into::into)
    }
}
impl WorkReader {
    fn visit_page(
        &self,
        expected: &wire::PageRef,
        remaining_read_bytes: u64,
        visitor: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> Result<GeneratedPageReadUsage> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io("finite reader state poisoned"))?;
        if let Some(page) = state.cache.get(&expected.object.digest) {
            // Keep the page borrowed under the cache mutex, so eviction cannot
            // leave per-worker uncharged retained Arc pages behind.
            visitor(page);
            increment(&mut state.statistics.page_cache_hits, 1)?;
            return Ok(GeneratedPageReadUsage {
                disk_bytes: 0,
                cache_hit: true,
            });
        }
        if expected.object.bytes > remaining_read_bytes {
            return Err(resource("finite lookup byte allowance"));
        }
        let frame = wire::read_object(
            &self.generation.directory,
            "pages",
            "rrwork",
            &expected.object,
            self.limits.max_page_bytes,
        )?;
        increment(&mut state.statistics.page_reads, 1)?;
        increment(&mut state.statistics.page_bytes_read, frame.len() as u64)?;
        let page = ValidatedWorkPage::decode_generated(
            &frame,
            expected,
            self.generation.execution(),
            self.limits.max_page_bytes,
            self.limits.max_page_records,
        )?;
        drop(frame);
        let payload = page.payload_bytes();
        visitor(&page);
        if !state.cache.can_store(
            &expected.object.digest,
            payload,
            self.limits.max_cached_page_bytes,
        ) {
            increment(&mut state.statistics.uncached_large_pages, 1)?;
        } else {
            while state
                .cache
                .needs_eviction(payload, self.limits.max_cached_page_bytes)
            {
                state.cache.evict()?;
                state.statistics.retained_page_payload_bytes = state.cache.page_bytes();
                state.statistics.retained_pages = state.cache.len();
                increment(&mut state.statistics.evictions, 1)?;
            }
            state.cache.insert(CachedPage {
                digest: expected.object.digest,
                page,
                bytes: payload,
            })?;
            state.statistics.retained_page_payload_bytes = state.cache.page_bytes();
        }
        state.statistics.retained_pages = state.cache.len();
        Ok(GeneratedPageReadUsage {
            disk_bytes: expected.object.bytes,
            cache_hit: false,
        })
    }
}
fn increment(value: &mut u64, amount: u64) -> Result<()> {
    *value = value
        .checked_add(amount)
        .ok_or_else(|| resource("finite reader statistic overflow"))?;
    Ok(())
}

impl ValidatedGeneration {
    /// Trust the checkpoint's generated provenance; validate integrity and
    /// bounded structure without replaying historical roots/windows or algebra.
    pub fn open_generated(
        directory: &Path,
        expected: &RawExecutionBinding,
        limits: StorageLimits,
    ) -> Result<Self> {
        wire::binding(expected, limits)?;
        check_directories(directory)?;
        let tip_bytes = wire::read(&directory.join(wire::CURRENT), wire::TIP_LIMIT)?;
        let tip: wire::Tip = wire::decode(wire::TIP_MAGIC, &tip_bytes, wire::TIP_LIMIT)?;
        drop(tip_bytes);
        let (manifest, state) = read_metadata(directory, &tip, expected, limits)?;
        // Required file metadata is bounded by the admitted page directory.
        // Content validation is lazy; no old Work is re-enumerated or reduced.
        for page in manifest.runs.iter().flatten() {
            let path = directory
                .join("pages")
                .join(wire::name(&page.object.digest, "rrwork"));
            let metadata = fs::symlink_metadata(path).map_err(io)?;
            if !metadata.is_file() || metadata.len() != page.object.bytes {
                return Err(corrupt(
                    "committed finite page missing/nonregular/wrong length",
                ));
            }
        }
        check_raw_files(directory, &manifest)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            digest: tip.manifest.digest,
            manifest: Arc::new(manifest),
            state: Arc::new(state),
        })
    }
}
/// Metadata decoding is shared, not completion authority. Callers separately
/// validate the file namespace appropriate to their distinct capability type.
pub(super) fn read_metadata(
    directory: &Path,
    tip: &wire::Tip,
    expected: &RawExecutionBinding,
    limits: StorageLimits,
) -> Result<(wire::Manifest, GenerationState)> {
    let bytes = wire::read_object(
        directory,
        "generations",
        "rrgen",
        &tip.manifest,
        limits.max_manifest_bytes,
    )?;
    let manifest: wire::Manifest =
        wire::decode(wire::MANIFEST_MAGIC, &bytes, limits.max_manifest_bytes)?;
    drop(bytes);
    let (records, _) = wire::validate_manifest(&manifest, expected, limits)?;
    if tip.generation != manifest.generation {
        return Err(corrupt("generation marker differs from manifest"));
    }
    let bytes = wire::read_object(
        directory,
        "states",
        "rrstate",
        &manifest.state,
        limits.max_state_bytes,
    )?;
    let state: wire::StateFile = wire::decode(wire::STATE_MAGIC, &bytes, limits.max_state_bytes)?;
    drop(bytes);
    if state.execution != *expected || state.generation != manifest.generation {
        return Err(corrupt("finite state execution/generation/scope mismatch"));
    }
    wire::validate_state(&state.state, expected, records, limits)?;
    if wire::scope_digest(&state.state.scope, limits.max_state_bytes)? != manifest.scope_digest {
        return Err(corrupt("finite state scope binding mismatch"));
    }
    Ok((manifest, state.state))
}
pub(super) fn check_raw_files(directory: &Path, manifest: &wire::Manifest) -> Result<()> {
    for page in manifest.raw.pages() {
        let metadata = fs::symlink_metadata(
            directory
                .join("raw")
                .join(wire::name(&page.object.digest, "rrraw")),
        )
        .map_err(io)?;
        if !metadata.is_file() || metadata.len() != page.object.bytes {
            return Err(corrupt(
                "committed raw page missing/nonregular/wrong length",
            ));
        }
    }
    Ok(())
}
pub(super) fn check_directories(directory: &Path) -> Result<()> {
    for path in [
        directory.to_path_buf(),
        directory.join("pages"),
        directory.join("raw"),
        directory.join("states"),
        directory.join("generations"),
    ] {
        let metadata = fs::symlink_metadata(path).map_err(io)?;
        if !metadata.is_dir() {
            return Err(corrupt(
                "finite-store directory is non-directory or symlink",
            ));
        }
    }
    Ok(())
}
