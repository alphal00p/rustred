use super::*;
use std::{collections::VecDeque, fs, path::Path, sync::Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Present,
    Absent,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReaderStatistics {
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
}
struct CachedPage {
    digest: Digest,
    records: Vec<StoredWork>,
    bytes: usize,
}
struct ReaderState {
    cache: VecDeque<CachedPage>,
    statistics: ReaderStatistics,
}

/// A lookup holds this reader's mutex through a bounded page read/decode. This
/// serializes backing I/O (not algebra) and bounds in-flight transient pages.
/// A zero resident cache allowance is supported: every hit is read exactly.
pub(crate) struct WorkReader {
    generation: ValidatedGeneration,
    limits: StorageLimits,
    state: Mutex<ReaderState>,
}
impl WorkReader {
    pub(super) fn new(generation: ValidatedGeneration, limits: StorageLimits) -> Result<Self> {
        let (records, _) =
            wire::validate_manifest(&generation.manifest, generation.execution(), limits)?;
        wire::validate_state(&generation.state, generation.execution(), records, limits)?;
        Ok(Self {
            generation,
            limits,
            state: Mutex::new(ReaderState {
                cache: VecDeque::new(),
                statistics: ReaderStatistics::default(),
            }),
        })
    }
    pub fn generation(&self) -> Digest {
        self.generation.digest
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
    pub fn lookup(&self, key: &StoredWork) -> Result<Lookup> {
        key.validate(self.generation.execution().physical_arity as usize)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io("finite reader state poisoned"))?;
        increment(&mut state.statistics.lookups, 1)?;
        let mut pages = 0usize;
        let mut bytes = 0u64;
        for run in &self.generation.manifest.runs {
            let index = run.partition_point(|page| &page.last < key);
            let Some(reference) = run.get(index).filter(|page| &page.first <= key) else {
                continue;
            };
            pages = pages
                .checked_add(1)
                .ok_or_else(|| resource("lookup page counter overflow"))?;
            if pages > self.limits.max_lookup_pages {
                return Err(resource("finite lookup page allowance"));
            }
            let found = if let Some(index) = state
                .cache
                .iter()
                .position(|page| page.digest == reference.object.digest)
            {
                let cached = state.cache.remove(index).expect("position exists");
                let found = cached.records.binary_search(key).is_ok();
                state.cache.push_back(cached);
                increment(&mut state.statistics.page_cache_hits, 1)?;
                found
            } else {
                bytes = bytes
                    .checked_add(reference.object.bytes)
                    .ok_or_else(|| resource("lookup byte counter overflow"))?;
                if bytes > self.limits.max_lookup_bytes {
                    return Err(resource("finite lookup byte allowance"));
                }
                let frame = wire::read_object(
                    &self.generation.directory,
                    "pages",
                    "rrwork",
                    &reference.object,
                    self.limits.max_page_bytes,
                )?;
                increment(&mut state.statistics.page_reads, 1)?;
                increment(&mut state.statistics.page_bytes_read, frame.len() as u64)?;
                let page: wire::Page =
                    wire::decode(wire::PAGE_MAGIC, &frame, self.limits.max_page_bytes)?;
                let payload = wire::validate_page(
                    &page,
                    reference,
                    self.generation.execution(),
                    self.limits,
                )?;
                let found = page.records.binary_search(key).is_ok();
                if self.limits.max_cached_pages == 0 || payload > self.limits.max_cached_page_bytes
                {
                    increment(&mut state.statistics.uncached_large_pages, 1)?;
                } else {
                    while state.cache.len() >= self.limits.max_cached_pages
                        || payload
                            > self
                                .limits
                                .max_cached_page_bytes
                                .saturating_sub(state.statistics.retained_page_payload_bytes)
                    {
                        let old = state
                            .cache
                            .pop_front()
                            .ok_or_else(|| corrupt("finite cache byte accounting"))?;
                        state.statistics.retained_page_payload_bytes -= old.bytes;
                        state.statistics.retained_pages = state.cache.len();
                        increment(&mut state.statistics.evictions, 1)?;
                    }
                    state.cache.try_reserve(1).map_err(resource)?;
                    state.statistics.retained_page_payload_bytes = state
                        .statistics
                        .retained_page_payload_bytes
                        .checked_add(payload)
                        .ok_or_else(|| resource("resident page byte overflow"))?;
                    state.cache.push_back(CachedPage {
                        digest: reference.object.digest,
                        records: page.records,
                        bytes: payload,
                    });
                }
                state.statistics.retained_pages = state.cache.len();
                found
            };
            if found {
                increment(&mut state.statistics.present, 1)?;
                return Ok(Lookup::Present);
            }
        }
        // Only exact range exclusions and fully validated candidate pages can
        // establish absence. No read/resource/corruption error reaches here.
        increment(&mut state.statistics.absent, 1)?;
        Ok(Lookup::Absent)
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
            return Err(corrupt("CURRENT generation differs from manifest"));
        }
        let state_bytes = wire::read_object(
            directory,
            "states",
            "rrstate",
            &manifest.state,
            limits.max_state_bytes,
        )?;
        let state: wire::StateFile =
            wire::decode(wire::STATE_MAGIC, &state_bytes, limits.max_state_bytes)?;
        drop(state_bytes);
        if state.execution != *expected || state.generation != manifest.generation {
            return Err(corrupt("finite state execution/generation/scope mismatch"));
        }
        wire::validate_state(&state.state, expected, records, limits)?;
        if wire::scope_digest(&state.state.scope, limits.max_state_bytes)? != manifest.scope_digest
        {
            return Err(corrupt("finite state scope binding mismatch"));
        }
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
        Ok(Self {
            directory: directory.to_path_buf(),
            digest: tip.manifest.digest,
            manifest: Arc::new(manifest),
            state: Arc::new(state.state),
        })
    }
}
pub(super) fn check_directories(directory: &Path) -> Result<()> {
    for path in [
        directory.to_path_buf(),
        directory.join("pages"),
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
