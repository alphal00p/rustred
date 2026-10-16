//! Stable slots keep promotion/eviction independent of the resident page count.
//! Only validated pages enter this cache; membership remains core's decision.
use super::*;
use std::{collections::HashMap, mem::size_of};

struct Slot {
    entry: Option<CachedPage>,
    previous: Option<usize>,
    next: Option<usize>,
}

#[derive(Default)]
pub(super) struct PageCache {
    // All captured page digests are installed once. Cache operations only
    // change slot values, so hash deletion/tombstones cannot grow this table.
    index: HashMap<Digest, Option<usize>>,
    slots: Vec<Slot>,
    oldest: Option<usize>,
    newest: Option<usize>,
    free: Option<usize>,
    max_entries: usize,
    retained: usize,
    metadata_bytes: usize,
    page_bytes: usize,
}

impl PageCache {
    pub(super) fn new(
        pages: usize,
        byte_grant: usize,
        runs: &[Vec<wire::PageRef>],
    ) -> Result<Self> {
        // Bound the requested allocations before reserve. Actual capacities
        // are charged below; hash control bytes/allocator slack are not RSS.
        if pages == 0 || byte_grant == 0 {
            return Ok(Self::default());
        }
        let count = runs
            .iter()
            .try_fold(0usize, |n, run| n.checked_add(run.len()))
            .ok_or_else(|| resource("page cache directory count overflow"))?;
        let index_estimate = count
            .checked_mul(size_of::<(Digest, Option<usize>)>())
            .ok_or_else(|| resource("page cache index overflow"))?;
        let mut requested =
            pages.min(byte_grant.saturating_sub(index_estimate) / size_of::<Slot>());
        while requested != 0 {
            let mut cache = Self::default();
            cache.slots.try_reserve_exact(requested).map_err(resource)?;
            cache.index.try_reserve(count).map_err(resource)?;
            cache.metadata_bytes = cache
                .slots
                .capacity()
                .checked_mul(size_of::<Slot>())
                .and_then(|bytes| {
                    cache
                        .index
                        .capacity()
                        .checked_mul(size_of::<(Digest, Option<usize>)>())
                        .and_then(|index| bytes.checked_add(index))
                })
                .ok_or_else(|| resource("page cache capacity overflow"))?;
            if cache.metadata_bytes < byte_grant {
                cache.max_entries = requested;
                for page in runs.iter().flatten() {
                    assert!(cache.index.insert(page.object.digest, None).is_none());
                }
                return Ok(cache);
            }
            // The optional cache can be smaller than its page ceiling. Do
            // not retain metadata which leaves no room for any page payload.
            drop(cache);
            requested /= 2;
        }
        Ok(Self::default())
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(super) fn page_bytes(&self) -> usize {
        self.page_bytes
    }
    pub(super) fn len(&self) -> usize {
        self.retained
    }
    pub(super) fn can_store(&self, digest: &Digest, bytes: usize, grant: usize) -> bool {
        self.max_entries != 0
            && self.index.contains_key(digest)
            && bytes <= grant.saturating_sub(self.metadata_bytes)
    }
    pub(super) fn needs_eviction(&self, bytes: usize, grant: usize) -> bool {
        self.len() >= self.max_entries
            || bytes
                > grant
                    .saturating_sub(self.metadata_bytes)
                    .saturating_sub(self.page_bytes)
    }
    pub(super) fn get(&mut self, digest: &Digest) -> Option<&ValidatedWorkPage> {
        let slot = (*self.index.get(digest)?)?;
        if self.newest != Some(slot) {
            self.detach(slot);
            self.append(slot);
        }
        Some(
            &self.slots[slot]
                .entry
                .as_ref()
                .expect("indexed live slot")
                .page,
        )
    }
    fn detach(&mut self, slot: usize) {
        let previous = self.slots[slot].previous;
        let next = self.slots[slot].next;
        if let Some(previous) = previous {
            self.slots[previous].next = next;
        } else {
            self.oldest = next;
        }
        if let Some(next) = next {
            self.slots[next].previous = previous;
        } else {
            self.newest = previous;
        }
    }
    fn append(&mut self, slot: usize) {
        self.slots[slot].previous = self.newest;
        self.slots[slot].next = None;
        if let Some(newest) = self.newest {
            self.slots[newest].next = Some(slot);
        } else {
            self.oldest = Some(slot);
        }
        self.newest = Some(slot);
    }
    pub(super) fn evict(&mut self) -> Result<()> {
        let slot = self
            .oldest
            .ok_or_else(|| corrupt("finite cache byte accounting"))?;
        self.detach(slot);
        let old = self.slots[slot].entry.take().expect("live oldest slot");
        let indexed = self
            .index
            .get_mut(&old.digest)
            .expect("captured page digest");
        assert_eq!(*indexed, Some(slot));
        *indexed = None;
        self.retained -= 1;
        self.page_bytes -= old.bytes;
        self.slots[slot].previous = None;
        self.slots[slot].next = self.free;
        self.free = Some(slot);
        Ok(())
    }
    pub(super) fn insert(&mut self, entry: CachedPage) -> Result<()> {
        let bytes = self
            .page_bytes
            .checked_add(entry.bytes)
            .ok_or_else(|| resource("resident page byte overflow"))?;
        let slot = if let Some(free) = self.free {
            self.free = self.slots[free].next;
            free
        } else {
            assert!(self.slots.len() < self.max_entries);
            let slot = self.slots.len();
            // Fully reserved at construction; no allocation on a cache hit
            // or insertion/eviction cycle.
            self.slots.push(Slot {
                entry: None,
                previous: None,
                next: None,
            });
            slot
        };
        let indexed = self
            .index
            .get_mut(&entry.digest)
            .expect("captured page digest");
        assert!(indexed.is_none());
        *indexed = Some(slot);
        self.retained += 1;
        self.slots[slot].entry = Some(entry);
        self.page_bytes = bytes;
        self.append(slot);
        Ok(())
    }
    pub(super) fn clear(&mut self) {
        for slot in self.index.values_mut() {
            *slot = None;
        }
        self.slots.clear();
        self.oldest = None;
        self.newest = None;
        self.free = None;
        self.page_bytes = 0;
        self.retained = 0;
        // Reserved capacities remain charged by metadata_bytes.
    }
    #[cfg(test)]
    pub(super) fn assert_consistent(&self) {
        let mut seen = std::collections::BTreeSet::new();
        let mut cursor = self.oldest;
        let mut previous = None;
        let mut bytes = 0;
        while let Some(slot) = cursor {
            assert!(seen.insert(slot));
            let value = &self.slots[slot];
            let entry = value.entry.as_ref().unwrap();
            assert_eq!(value.previous, previous);
            assert_eq!(self.index[&entry.digest], Some(slot));
            bytes += entry.bytes;
            previous = cursor;
            cursor = value.next;
        }
        assert_eq!(previous, self.newest);
        assert_eq!(seen.len(), self.retained);
        assert_eq!(bytes, self.page_bytes);
        assert_eq!(
            self.index.values().filter(|v| v.is_some()).count(),
            self.retained
        );
        cursor = self.free;
        while let Some(slot) = cursor {
            assert!(seen.insert(slot));
            assert!(self.slots[slot].entry.is_none());
            cursor = self.slots[slot].next;
        }
        assert_eq!(seen.len(), self.slots.len());
        assert!(self.slots.len() <= self.max_entries);
    }
}
