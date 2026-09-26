//! Optional completed-result storage, not publication or completed coverage.
//! Only successful, non-running slots move here. Their final flush has returned,
//! so each holds at most one published chunk; no running stream is accumulated.
use std::collections::HashMap;

use super::{Event, Finished, Slot};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub entries: usize,
    pub bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 65_536,
            bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

pub(super) struct Entry<const N: usize> {
    pub chunk: Option<Vec<Event<N>>>,
    pub finished: Finished,
    charge: Charge,
}
#[derive(Clone, Copy)]
pub(super) struct Charge {
    pub bytes: usize,
    chunk_bytes: usize,
    pub payload: usize,
    pub events: usize,
}
pub(super) struct Escrow<const N: usize> {
    entries: HashMap<usize, Entry<N>>,
    pub limits: Limits,
    pub bytes: usize,
    pub payload: usize,
    pub events: usize,
    pub peak_bytes: usize,
    pub peak_entries: usize,
    pub reclaimed: usize,
    pub reserve_failed: bool,
}
impl<const N: usize> Escrow<N> {
    pub fn new(limits: Limits) -> Self {
        Self {
            entries: HashMap::new(),
            limits,
            bytes: 0,
            payload: 0,
            events: 0,
            peak_bytes: 0,
            peak_entries: 0,
            reclaimed: 0,
            reserve_failed: false,
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn contains(&self, id: usize) -> bool {
        self.entries.contains_key(&id)
    }
    pub fn eligible(slot: &Slot<N>) -> bool {
        !slot.running
            && slot.job.is_none()
            && slot.finished.as_ref().is_some_and(|f| f.error.is_none())
    }
    pub fn charge(slot: &Slot<N>) -> Option<Charge> {
        if !Self::eligible(slot) {
            return None;
        }
        let mut payload = 0usize;
        let mut events = 0usize;
        let mut spare = 0usize;
        if let Some(chunk) = &slot.chunk {
            for event in chunk {
                payload = payload.checked_add(event.weight())?;
                events = events.checked_add(event.count)?;
            }
            spare = (chunk.capacity() - chunk.len()).checked_mul(size_of::<Event<N>>())?;
        }
        let chunk_bytes = payload.checked_add(spare)?;
        Some(Charge {
            // Includes entry metadata and Vec spare storage. HashMap spare
            // buckets/allocator overhead remain additional, bounded by the
            // entry ceiling; this is an accounted-storage cap, not RSS.
            bytes: chunk_bytes.checked_add(size_of::<(usize, Entry<N>)>())?,
            chunk_bytes,
            payload,
            events,
        })
    }
    /// Whether any further entry can be admitted at all: a failed allocation
    /// or the entry cap refuses every charge, so callers stop scanning slots.
    pub fn has_room(&self) -> bool {
        !self.reserve_failed && self.len() < self.limits.entries
    }
    pub fn fits(&self, charge: Charge) -> bool {
        self.has_room()
            && self.reclaimed < usize::MAX
            && self
                .bytes
                .checked_add(charge.bytes)
                .is_some_and(|n| n <= self.limits.bytes)
            && self.payload.checked_add(charge.payload).is_some()
            && self.events.checked_add(charge.events).is_some()
    }
    pub fn reserve(&mut self, charge: Charge) -> bool {
        if !self.fits(charge) {
            return false;
        }
        if self.entries.try_reserve(1).is_err() {
            // Optional optimization: leave the slot untouched and use ordinary
            // ordered polling. Do not repeatedly retry a failed allocation.
            self.reserve_failed = true;
            return false;
        }
        true
    }
    pub fn insert(&mut self, id: usize, slot: &mut Slot<N>, charge: Charge) {
        debug_assert!(!self.entries.contains_key(&id));
        self.entries.insert(
            id,
            Entry {
                chunk: slot.chunk.take(),
                finished: slot.finished.take().expect("admitted finished escrow"),
                charge,
            },
        );
        self.bytes += charge.bytes;
        self.payload += charge.payload;
        self.events += charge.events;
        self.reclaimed += 1;
        self.peak_bytes = self.peak_bytes.max(self.bytes);
        self.peak_entries = self.peak_entries.max(self.len());
        slot.id = None;
        slot.phase = None;
    }
    pub fn take_chunk(&mut self, id: usize) -> Option<Vec<Event<N>>> {
        let entry = self.entries.get_mut(&id)?;
        let chunk = entry.chunk.take()?;
        self.bytes -= entry.charge.chunk_bytes;
        self.payload -= entry.charge.payload;
        self.events -= entry.charge.events;
        entry.charge.bytes -= entry.charge.chunk_bytes;
        entry.charge.chunk_bytes = 0;
        entry.charge.payload = 0;
        entry.charge.events = 0;
        Some(chunk)
    }
    pub fn take_finished(&mut self, id: usize) -> Option<Finished> {
        if self.entries.get(&id)?.chunk.is_some() {
            return None;
        }
        let entry = self.entries.remove(&id)?;
        self.bytes -= entry.charge.bytes;
        Some(entry.finished)
    }
    pub fn clear_chunks(&mut self, mut release: impl FnMut(&[Event<N>])) {
        for entry in self.entries.values_mut() {
            if let Some(chunk) = entry.chunk.take() {
                release(&chunk);
                self.bytes -= entry.charge.chunk_bytes;
                entry.charge.bytes -= entry.charge.chunk_bytes;
                entry.charge.chunk_bytes = 0;
                entry.charge.payload = 0;
                entry.charge.events = 0;
            }
        }
        self.payload = 0;
        self.events = 0;
    }
    pub fn drain(&mut self) -> impl Iterator<Item = (usize, Entry<N>)> + '_ {
        self.bytes = 0;
        self.payload = 0;
        self.events = 0;
        self.entries.drain()
    }
}
