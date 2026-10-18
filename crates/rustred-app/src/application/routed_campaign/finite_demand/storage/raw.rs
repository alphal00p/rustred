//! Typed raw terminal/zero pages. These can NEVER answer completed-Work
//! membership: phase-qualified Work uses a different codec and directory.
use super::*;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap, VecDeque},
    mem::size_of,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const MAGIC: &[u8] = b"RUSTRED-FINITE-RAW-KEYS-1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, bincode::Encode, bincode::Decode)]
pub(crate) enum RawKeyKind {
    Terminal,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawCensusLimits {
    pub max_page_records: usize,
    pub max_page_bytes: usize,
    pub max_runs: usize,
    pub max_pages: usize,
    pub max_directory_bytes: usize,
    /// References may repeat across windows; this is NOT a distinct count.
    pub max_stored_references: u64,
    pub max_merge_runs: usize,
    pub max_merge_cached_pages: usize,
    pub max_merge_payload_bytes: usize,
    pub max_merge_read_bytes: u64,
    /// Separate encoded+native decoder transient allowance, not resident
    /// merge payload or a claim about total process RSS.
    pub max_decode_transient_bytes: usize,
}
impl Default for RawCensusLimits {
    fn default() -> Self {
        Self {
            max_page_records: 128,
            max_page_bytes: 2 * 1024 * 1024,
            max_runs: 4096,
            max_pages: 1_000_000,
            max_directory_bytes: 256 * 1024 * 1024,
            max_stored_references: 1_000_000_000,
            max_merge_runs: 4096,
            max_merge_cached_pages: 1024,
            max_merge_payload_bytes: 256 * 1024 * 1024,
            max_merge_read_bytes: 128 * 1024 * 1024 * 1024,
            max_decode_transient_bytes: wire::MAX_FRAME + 2 * 1024 * 1024,
        }
    }
}
impl RawCensusLimits {
    /// Largest record grant admitted by the EXISTING decoded raw-page policy.
    /// This is not inferred from the encoded frame size: both writer capacity
    /// admission and directory payload validation enforce this decoded bound.
    pub(crate) fn max_records_for_native_frame(arity: usize) -> Result<usize> {
        let per_record = raw_record_payload(arity)?;
        let records = wire::MAX_FRAME / per_record;
        if records == 0 {
            return Err(resource("raw record exceeds native decoder allowance"));
        }
        Ok(records)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(super) struct RawPageRef {
    pub kind: RawKeyKind,
    pub object: wire::ObjectRef,
    pub records: u64,
    pub payload_bytes: u64,
    pub first: Vec<i64>,
    pub last: Vec<i64>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct RawCensusDirectory {
    pub(super) terminals: Vec<Vec<RawPageRef>>,
    pub(super) zeros: Vec<Vec<RawPageRef>>,
}
impl RawCensusDirectory {
    pub(super) fn runs(&self, kind: RawKeyKind) -> &[Vec<RawPageRef>] {
        match kind {
            RawKeyKind::Terminal => &self.terminals,
            RawKeyKind::Zero => &self.zeros,
        }
    }
    pub(super) fn runs_mut(&mut self, kind: RawKeyKind) -> &mut Vec<Vec<RawPageRef>> {
        match kind {
            RawKeyKind::Terminal => &mut self.terminals,
            RawKeyKind::Zero => &mut self.zeros,
        }
    }
    pub fn stored_references(&self, kind: RawKeyKind) -> Result<u64> {
        self.runs(kind).iter().flatten().try_fold(0u64, |sum, p| {
            sum.checked_add(p.records)
                .ok_or_else(|| resource("raw reference count overflow"))
        })
    }
    pub(super) fn pages(&self) -> impl Iterator<Item = &RawPageRef> {
        self.terminals.iter().chain(&self.zeros).flatten()
    }
}
#[derive(Debug, bincode::Encode, bincode::Decode)]
pub(super) struct RawPage {
    pub execution: RawExecutionBinding,
    pub kind: RawKeyKind,
    pub keys: Vec<Vec<i64>>,
}

/// Actual retained descriptor/vector payload; page scratch is separate.
pub(super) fn directory_payload(directory: &RawCensusDirectory) -> Result<usize> {
    let mut bytes = size_of::<RawCensusDirectory>();
    for runs in [&directory.terminals, &directory.zeros] {
        bytes = bytes
            .checked_add(
                runs.capacity()
                    .checked_mul(size_of::<Vec<RawPageRef>>())
                    .ok_or_else(|| resource("raw run payload overflow"))?,
            )
            .ok_or_else(|| resource("raw directory payload overflow"))?;
        for run in runs {
            bytes = bytes
                .checked_add(
                    run.capacity()
                        .checked_mul(size_of::<RawPageRef>())
                        .ok_or_else(|| resource("raw descriptor payload overflow"))?,
                )
                .ok_or_else(|| resource("raw directory payload overflow"))?;
            for page in run {
                let keys = page
                    .first
                    .capacity()
                    .checked_add(page.last.capacity())
                    .and_then(|n| n.checked_mul(size_of::<i64>()))
                    .ok_or_else(|| resource("raw key descriptor payload overflow"))?;
                bytes = bytes
                    .checked_add(keys)
                    .ok_or_else(|| resource("raw directory payload overflow"))?;
            }
        }
    }
    Ok(bytes)
}

/// Borrowed sorted native delta sets: only one output page is copied at a time.
pub(super) struct RawDelta<'a> {
    pub terminals: Box<dyn Iterator<Item = &'a [i64]> + 'a>,
    pub zeros: Box<dyn Iterator<Item = &'a [i64]> + 'a>,
}
impl RawDelta<'_> {
    pub fn empty() -> Self {
        Self {
            terminals: Box::new(std::iter::empty()),
            zeros: Box::new(std::iter::empty()),
        }
    }
}
impl<'a> RawDelta<'a> {
    pub fn native<const N: usize>(
        report: &'a rustred::solver::CandidateRoutedCampaignReport<N>,
    ) -> Self {
        Self {
            terminals: Box::new(
                report
                    .trace()
                    .declared_terminals()
                    .iter()
                    .map(|k| k.powers()),
            ),
            zeros: Box::new(report.trace().visited_zeros().iter().map(|k| k.powers())),
        }
    }
    #[cfg(test)]
    pub fn slices(terminals: &'a [Vec<i64>], zeros: &'a [Vec<i64>]) -> Self {
        Self {
            terminals: Box::new(terminals.iter().map(Vec::as_slice)),
            zeros: Box::new(zeros.iter().map(Vec::as_slice)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RawCensusSummary {
    pub terminals: u64,
    pub zeros: u64,
    pub pages_read: u64,
    pub bytes_read: u64,
    pub page_cache_hits: u64,
    pub page_evictions: u64,
    pub peak_payload_bytes: usize,
    pub decode_transient_allowance_bytes: usize,
}

pub(super) fn key_payload(keys: &[Vec<i64>], capacity: usize) -> Result<usize> {
    keys.iter().try_fold(
        capacity
            .checked_mul(size_of::<Vec<i64>>())
            .ok_or_else(|| resource("raw page capacity overflow"))?,
        |bytes, key| {
            bytes
                .checked_add(
                    key.capacity()
                        .checked_mul(size_of::<i64>())
                        .ok_or_else(|| resource("raw key payload overflow"))?,
                )
                .ok_or_else(|| resource("raw page payload overflow"))
        },
    )
}
pub(super) fn validate_directory(
    directory: &RawCensusDirectory,
    arity: usize,
    limits: RawCensusLimits,
) -> Result<(usize, u64)> {
    if limits.max_page_records == 0 || limits.max_page_bytes == 0 {
        return Err(resource("raw page framing policy"));
    }
    page_capacity_payload(arity, limits)?;
    if wire::metadata_decode_allowance(limits.max_page_bytes)
        .checked_add(limits.max_page_bytes)
        .is_none_or(|n| n > limits.max_decode_transient_bytes)
    {
        return Err(resource("raw native decoder transient allowance"));
    }
    let mut pages = 0usize;
    let mut refs = 0u64;
    let mut bytes = 0u64;
    let mut payload = size_of::<RawCensusDirectory>();
    let mut digests = BTreeMap::new();
    for kind in [RawKeyKind::Terminal, RawKeyKind::Zero] {
        let runs = directory.runs(kind);
        let run_capacity = match kind {
            RawKeyKind::Terminal => directory.terminals.capacity(),
            RawKeyKind::Zero => directory.zeros.capacity(),
        };
        if runs.len() > limits.max_runs {
            return Err(resource("raw run directory allowance"));
        }
        payload = payload
            .checked_add(
                run_capacity
                    .checked_mul(size_of::<Vec<RawPageRef>>())
                    .ok_or_else(|| resource("raw run payload overflow"))?,
            )
            .ok_or_else(|| resource("raw directory overflow"))?;
        for run in runs {
            if run.is_empty() {
                return Err(corrupt("empty raw key run"));
            }
            payload = payload
                .checked_add(
                    run.capacity()
                        .checked_mul(size_of::<RawPageRef>())
                        .ok_or_else(|| resource("raw page directory capacity overflow"))?,
                )
                .ok_or_else(|| resource("raw directory payload overflow"))?;
            let mut previous = None;
            for page in run {
                if page.kind != kind
                    || page.records == 0
                    || page.first.len() != arity
                    || page.last.len() != arity
                    || page.first > page.last
                    || previous.is_some_and(|old| old >= &page.first)
                    || digests
                        .insert(page.object.digest, page)
                        .is_some_and(|old| old != page)
                {
                    return Err(corrupt("raw page kind/range/count/digest directory"));
                }
                if page.records > limits.max_page_records as u64
                    || page.object.bytes == 0
                    || page.object.bytes > limits.max_page_bytes as u64
                    || page.payload_bytes
                        > wire::metadata_decode_allowance(limits.max_page_bytes) as u64
                {
                    return Err(resource("raw page descriptor allowance"));
                }
                previous = Some(&page.last);
                pages = pages
                    .checked_add(1)
                    .ok_or_else(|| resource("raw page count overflow"))?;
                refs = refs
                    .checked_add(page.records)
                    .ok_or_else(|| resource("raw reference count overflow"))?;
                bytes = bytes
                    .checked_add(page.object.bytes)
                    .ok_or_else(|| resource("raw referenced bytes overflow"))?;
                payload = payload
                    .checked_add(size_of::<(Digest, &RawPageRef)>())
                    .and_then(|n| {
                        arity
                            .checked_mul(2 * size_of::<i64>())
                            .and_then(|k| n.checked_add(k))
                    })
                    .ok_or_else(|| resource("raw directory payload overflow"))?;
                if pages > limits.max_pages
                    || refs > limits.max_stored_references
                    || payload > limits.max_directory_bytes
                {
                    return Err(resource("raw directory/reference allowance"));
                }
            }
        }
    }
    if payload > limits.max_directory_bytes {
        return Err(resource("raw directory resident allowance"));
    }
    Ok((pages, bytes))
}
fn raw_record_payload(arity: usize) -> Result<usize> {
    if arity == 0 {
        return Err(resource("raw page physical arity must be positive"));
    }
    arity
        .checked_mul(size_of::<i64>())
        .and_then(|n| n.checked_add(size_of::<Vec<i64>>()))
        .ok_or_else(|| resource("raw record payload allowance overflow"))
}
pub(super) fn page_capacity_payload(arity: usize, limits: RawCensusLimits) -> Result<usize> {
    let payload = raw_record_payload(arity)?
        .checked_mul(limits.max_page_records)
        .ok_or_else(|| resource("raw page capacity allowance overflow"))?;
    // Follow the caller-selected frame/decoder budget. The own-decoder
    // roundtrip remains the final frame gate before durable installation.
    if payload > wire::metadata_decode_allowance(limits.max_page_bytes) {
        return Err(resource(
            "raw page capacity exceeds native decoder allowance",
        ));
    }
    Ok(payload)
}
pub(super) fn decode_page(
    directory: &PathBuf,
    reference: &RawPageRef,
    execution: &RawExecutionBinding,
    limits: RawCensusLimits,
) -> Result<RawPage> {
    let bytes = wire::read_object(
        directory,
        "raw",
        "rrraw",
        &reference.object,
        limits.max_page_bytes,
    )?;
    decode_page_bytes(&bytes, reference, execution, limits)
}
pub(super) fn decode_page_bytes(
    bytes: &[u8],
    reference: &RawPageRef,
    execution: &RawExecutionBinding,
    limits: RawCensusLimits,
) -> Result<RawPage> {
    if reference.records > limits.max_page_records as u64 {
        return Err(resource("raw page record allowance"));
    }
    if bytes.len() as u64 != reference.object.bytes
        || blake3::hash(bytes).as_bytes() != &reference.object.digest
    {
        return Err(corrupt("raw page object hash/length mismatch"));
    }
    let page: RawPage = wire::decode(MAGIC, bytes, limits.max_page_bytes)?;
    if page.execution != *execution
        || page.kind != reference.kind
        || page.keys.len() as u64 != reference.records
        || page.keys.first() != Some(&reference.first)
        || page.keys.last() != Some(&reference.last)
        || page.keys.windows(2).any(|p| p[0] >= p[1])
        || page
            .keys
            .iter()
            .any(|k| k.len() != execution.physical_arity as usize)
        || key_payload(&page.keys, page.keys.capacity())? as u64 != reference.payload_bytes
    {
        return Err(corrupt(
            "raw page execution/kind/range/payload/ordering mismatch",
        ));
    }
    Ok(page)
}

struct CachedRaw {
    reference: RawPageRef,
    page: RawPage,
    payload: usize,
}
#[derive(Clone, Copy)]
pub(super) struct RawView<'a> {
    pub directory: &'a PathBuf,
    pub execution: &'a RawExecutionBinding,
    pub raw: &'a RawCensusDirectory,
}
struct PageCache<'a> {
    source: RawView<'a>,
    limits: RawCensusLimits,
    metadata: usize,
    pages: VecDeque<CachedRaw>,
    resident: usize,
    summary: RawCensusSummary,
}
impl PageCache<'_> {
    fn at(
        &mut self,
        reference: &RawPageRef,
        index: usize,
        cancel: &AtomicBool,
        classify: &mut dyn FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<Vec<i64>> {
        if cancel.load(Ordering::Acquire) {
            return Err(StorageError::new(
                ErrorKind::Cancelled,
                "raw census merge cancelled",
            ));
        }
        if let Some(position) = self
            .pages
            .iter()
            .position(|p| p.reference.object.digest == reference.object.digest)
        {
            let page = self.pages.remove(position).expect("position exists");
            if page.reference != *reference {
                return Err(corrupt("cached raw page descriptor changed"));
            }
            let key = page
                .page
                .keys
                .get(index)
                .ok_or_else(|| corrupt("raw page cursor range"))?
                .clone();
            self.pages.push_back(page);
            self.summary.page_cache_hits += 1;
            return Ok(key);
        }
        let read = self
            .summary
            .bytes_read
            .checked_add(reference.object.bytes)
            .ok_or_else(|| resource("raw merge read counter overflow"))?;
        if read > self.limits.max_merge_read_bytes {
            return Err(resource("raw census merge read allowance"));
        }
        let payload = usize::try_from(reference.payload_bytes).map_err(resource)?;
        if payload
            > self
                .limits
                .max_merge_payload_bytes
                .saturating_sub(self.metadata)
        {
            return Err(resource("raw page exceeds merge residency allowance"));
        }
        while !self.pages.is_empty()
            && (self.pages.len() >= self.limits.max_merge_cached_pages
                || payload
                    > self
                        .limits
                        .max_merge_payload_bytes
                        .saturating_sub(self.metadata + self.resident))
        {
            let old = self.pages.pop_front().unwrap();
            self.resident -= old.payload;
            self.summary.page_evictions += 1;
        }
        let page = decode_page(
            self.source.directory,
            reference,
            self.source.execution,
            self.limits,
        )?;
        // Source classification runs only on a freshly decoded page, not
        // every key/head lookup. Evicted pages are safely revalidated on read.
        for key in &page.keys {
            if cancel.load(Ordering::Acquire) {
                return Err(StorageError::new(
                    ErrorKind::Cancelled,
                    "raw page classification cancelled",
                ));
            }
            classify(reference.kind, key)?;
        }
        self.summary.bytes_read = read;
        self.summary.pages_read += 1;
        self.summary.peak_payload_bytes = self
            .summary
            .peak_payload_bytes
            .max(self.metadata + self.resident + payload);
        let key = page
            .keys
            .get(index)
            .ok_or_else(|| corrupt("raw page cursor range"))?
            .clone();
        if self.limits.max_merge_cached_pages != 0 {
            self.pages.try_reserve(1).map_err(resource)?;
            self.resident += payload;
            self.pages.push_back(CachedRaw {
                reference: reference.clone(),
                page,
                payload,
            });
        }
        Ok(key)
    }
}
struct RunCursor<'a> {
    descriptors: &'a [RawPageRef],
    page: usize,
    index: usize,
}
struct Merge<'a> {
    runs: Vec<RunCursor<'a>>,
    heap: BinaryHeap<Reverse<(Vec<i64>, usize)>>,
    last: Option<Vec<i64>>,
}
impl<'a> Merge<'a> {
    fn new(runs: &'a [Vec<RawPageRef>]) -> Result<Self> {
        let mut result = Self {
            runs: Vec::new(),
            heap: BinaryHeap::new(),
            last: None,
        };
        result
            .runs
            .try_reserve_exact(runs.len())
            .map_err(resource)?;
        result
            .heap
            .try_reserve_exact(runs.len())
            .map_err(resource)?;
        for run in runs {
            result.runs.push(RunCursor {
                descriptors: run,
                page: 0,
                index: 0,
            });
        }
        Ok(result)
    }
    fn fill(
        &mut self,
        index: usize,
        cache: &mut PageCache<'_>,
        cancel: &AtomicBool,
        classify: &mut dyn FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<()> {
        let cursor = &mut self.runs[index];
        if cursor
            .descriptors
            .get(cursor.page)
            .is_some_and(|p| cursor.index as u64 == p.records)
        {
            cursor.page += 1;
            cursor.index = 0;
        }
        let Some(reference) = cursor.descriptors.get(cursor.page) else {
            return Ok(());
        };
        let key = cache.at(reference, cursor.index, cancel, classify)?;
        cursor.index += 1;
        self.heap.push(Reverse((key, index)));
        Ok(())
    }
    fn prime(
        &mut self,
        cache: &mut PageCache<'_>,
        cancel: &AtomicBool,
        classify: &mut dyn FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<()> {
        for i in 0..self.runs.len() {
            self.fill(i, cache, cancel, classify)?;
        }
        Ok(())
    }
    fn next(
        &mut self,
        cache: &mut PageCache<'_>,
        cancel: &AtomicBool,
        classify: &mut dyn FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<Option<Vec<i64>>> {
        while let Some(Reverse((key, index))) = self.heap.pop() {
            self.fill(index, cache, cancel, classify)?;
            if self.last.as_ref() == Some(&key) {
                continue;
            }
            self.last = Some(key.clone());
            return Ok(Some(key));
        }
        Ok(None)
    }
}
impl ValidatedGeneration {
    pub fn raw_census_directory(&self) -> &RawCensusDirectory {
        &self.manifest.raw
    }
    pub fn visit_raw_census(
        &self,
        limits: RawCensusLimits,
        cancel: &AtomicBool,
        classify: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
        visitor: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<RawCensusSummary> {
        RawView {
            directory: &self.directory,
            execution: self.execution(),
            raw: &self.manifest.raw,
        }
        .visit_raw_census(limits, cancel, classify, visitor)
    }
}
impl RawView<'_> {
    /// Stream every referenced page and the sorted exact union. Conflicts
    /// between raw terminal/zero classifications fail, including across runs.
    /// The callback owns source classification; integrity alone is not proof.
    pub(super) fn visit_raw_census(
        &self,
        limits: RawCensusLimits,
        cancel: &AtomicBool,
        mut classify: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
        mut visitor: impl FnMut(RawKeyKind, &[i64]) -> Result<()>,
    ) -> Result<RawCensusSummary> {
        if cancel.load(Ordering::Acquire) {
            return Err(StorageError::new(
                ErrorKind::Cancelled,
                "raw census iteration cancelled",
            ));
        }
        validate_directory(self.raw, self.execution.physical_arity as usize, limits)?;
        let runs = self
            .raw
            .terminals
            .len()
            .checked_add(self.raw.zeros.len())
            .ok_or_else(|| resource("raw merge run count overflow"))?;
        if runs > limits.max_merge_runs {
            return Err(resource("raw exact merge fan-in allowance"));
        }
        let arity = self.execution.physical_arity as usize;
        // One full key/head and exact page/row cursor per run, never one whole
        // page per run. Four output/dedup key buffers are charged separately.
        let metadata = runs
            .checked_mul(
                size_of::<RunCursor>() + size_of::<(Vec<i64>, usize)>() + arity * size_of::<i64>(),
            )
            .and_then(|n| n.checked_add(4 * arity * size_of::<i64>()))
            .and_then(|n| {
                limits
                    .max_merge_cached_pages
                    .checked_mul(size_of::<CachedRaw>() + 2 * arity * size_of::<i64>())
                    .and_then(|k| n.checked_add(k))
            })
            .ok_or_else(|| resource("raw merge metadata overflow"))?;
        if metadata > limits.max_merge_payload_bytes {
            return Err(resource("raw exact merge head allowance"));
        }
        let mut cache = PageCache {
            source: *self,
            limits,
            metadata,
            pages: VecDeque::new(),
            resident: 0,
            summary: RawCensusSummary::default(),
        };
        let mut terminals = Merge::new(&self.raw.terminals)?;
        let mut zeros = Merge::new(&self.raw.zeros)?;
        cache
            .pages
            .try_reserve_exact(limits.max_merge_cached_pages)
            .map_err(resource)?;
        let cursor_capacity = terminals
            .runs
            .capacity()
            .checked_add(zeros.runs.capacity())
            .ok_or_else(|| resource("raw merge cursor capacity overflow"))?;
        let heap_capacity = terminals
            .heap
            .capacity()
            .checked_add(zeros.heap.capacity())
            .ok_or_else(|| resource("raw merge heap capacity overflow"))?;
        let actual = cursor_capacity
            .checked_mul(size_of::<RunCursor>())
            .and_then(|n| {
                heap_capacity
                    .checked_mul(size_of::<(Vec<i64>, usize)>())
                    .and_then(|k| n.checked_add(k))
            })
            .and_then(|n| {
                runs.checked_add(6)
                    .and_then(|k| k.checked_mul(arity))
                    .and_then(|k| k.checked_mul(size_of::<i64>()))
                    .and_then(|k| n.checked_add(k))
            })
            .and_then(|n| {
                cache
                    .pages
                    .capacity()
                    .checked_mul(size_of::<CachedRaw>() + 2 * arity * size_of::<i64>())
                    .and_then(|k| n.checked_add(k))
            })
            .ok_or_else(|| resource("raw merge actual capacity overflow"))?;
        if actual > limits.max_merge_payload_bytes {
            return Err(resource("raw merge actual capacity allowance"));
        }
        cache.metadata = actual;
        terminals.prime(&mut cache, cancel, &mut classify)?;
        zeros.prime(&mut cache, cancel, &mut classify)?;
        let mut t = terminals.next(&mut cache, cancel, &mut classify)?;
        let mut z = zeros.next(&mut cache, cancel, &mut classify)?;
        let mut result = RawCensusSummary::default();
        while t.is_some() || z.is_some() {
            let kind = match (&t, &z) {
                (Some(t), Some(z)) => match t.cmp(z) {
                    std::cmp::Ordering::Equal => {
                        return Err(corrupt("raw terminal/zero union conflict"));
                    }
                    std::cmp::Ordering::Less => RawKeyKind::Terminal,
                    std::cmp::Ordering::Greater => RawKeyKind::Zero,
                },
                (Some(_), None) => RawKeyKind::Terminal,
                (None, Some(_)) => RawKeyKind::Zero,
                (None, None) => unreachable!(),
            };
            match kind {
                RawKeyKind::Terminal => {
                    visitor(kind, t.as_ref().unwrap())?;
                    result.terminals += 1;
                    t = terminals.next(&mut cache, cancel, &mut classify)?;
                }
                RawKeyKind::Zero => {
                    visitor(kind, z.as_ref().unwrap())?;
                    result.zeros += 1;
                    z = zeros.next(&mut cache, cancel, &mut classify)?;
                }
            }
        }
        result.pages_read = cache.summary.pages_read;
        result.bytes_read = cache.summary.bytes_read;
        result.page_cache_hits = cache.summary.page_cache_hits;
        result.page_evictions = cache.summary.page_evictions;
        result.peak_payload_bytes = cache.summary.peak_payload_bytes;
        result.decode_transient_allowance_bytes = limits.max_decode_transient_bytes;
        if cancel.load(Ordering::Acquire) {
            return Err(StorageError::new(
                ErrorKind::Cancelled,
                "raw census final validation cancelled",
            ));
        }
        Ok(result)
    }
}
