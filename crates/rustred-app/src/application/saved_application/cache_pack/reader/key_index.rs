//! Optional exact metadata for one immutable reader snapshot, never algebra.
use rustred::persistence::application_cache::ApplicationCacheKey;
use std::mem::size_of;

type Slot = (usize, usize);

struct Entry {
    slot: Slot,
    keys: Vec<ApplicationCacheKey>,
}

#[derive(Default)]
pub(super) struct KeyIndex {
    entries: Vec<Entry>,
    key_bytes: usize,
}

impl KeyIndex {
    pub(super) fn owned_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<Entry>() + self.key_bytes
    }

    pub(super) fn contains(&self, slot: Slot, key: &ApplicationCacheKey) -> Option<bool> {
        self.entries
            .binary_search_by_key(&slot, |entry| entry.slot)
            .ok()
            .map(|index| self.entries[index].keys.binary_search(key).is_ok())
    }

    /// Budget or allocation refusal simply leaves the ordinary reader active.
    /// Charge actual vector capacities, key-owned heaps and slot bookkeeping.
    pub(super) fn retain<'a>(
        &mut self,
        slot: Slot,
        keys: impl ExactSizeIterator<Item = &'a ApplicationCacheKey> + Clone,
        allowance: usize,
    ) {
        if allowance <= self.owned_bytes() {
            return;
        }
        let Err(position) = self.entries.binary_search_by_key(&slot, |entry| entry.slot) else {
            return;
        };
        let Some(next_len) = self.entries.len().checked_add(1) else {
            return;
        };
        let minimum_slots = self.entries.capacity().max(next_len);
        let Some(minimum) = keys
            .len()
            .checked_mul(size_of::<ApplicationCacheKey>())
            .and_then(|bytes| {
                keys.clone().try_fold(bytes, |bytes, key| {
                    bytes.checked_add(key.owned_heap_bytes())
                })
            })
            .and_then(|bytes| bytes.checked_add(self.key_bytes))
            .and_then(|bytes| {
                minimum_slots
                    .checked_mul(size_of::<Entry>())
                    .and_then(|slots| bytes.checked_add(slots))
            })
        else {
            return;
        };
        if minimum > allowance {
            return;
        }
        let mut owned = Vec::new();
        if owned.try_reserve_exact(keys.len()).is_err() {
            return;
        }
        owned.extend(keys.cloned());
        let Some(key_bytes) = owned
            .capacity()
            .checked_mul(size_of::<ApplicationCacheKey>())
            .and_then(|bytes| {
                owned.iter().try_fold(bytes, |bytes, key| {
                    bytes.checked_add(key.owned_heap_bytes())
                })
            })
            .and_then(|bytes| bytes.checked_add(self.key_bytes))
        else {
            return;
        };
        let Some(metadata_allowance) = allowance.checked_sub(key_bytes) else {
            return;
        };
        let maximum_slots = metadata_allowance / size_of::<Entry>();
        if maximum_slots < minimum_slots {
            return;
        }
        if next_len > self.entries.capacity() {
            let capacity = self
                .entries
                .capacity()
                .saturating_mul(2)
                .max(4)
                .max(next_len)
                .min(maximum_slots);
            // Allocate before moving anything so an unexpectedly larger Vec
            // capacity or allocation refusal cannot inflate the retained census.
            let mut grown = Vec::new();
            if grown.try_reserve_exact(capacity).is_err() || grown.capacity() > maximum_slots {
                return;
            }
            grown.append(&mut self.entries);
            self.entries = grown;
        }
        self.entries.insert(position, Entry { slot, keys: owned });
        self.key_bytes = key_bytes;
        debug_assert!(self.owned_bytes() <= allowance);
    }
}

#[cfg(test)]
mod tests;
