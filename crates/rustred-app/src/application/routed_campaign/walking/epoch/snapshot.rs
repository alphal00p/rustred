//! Bounded immutable lookup leases, independent of the coordinator's Store.
//!
//! Two lookup-only buffers are initialized once. A buffer is advanced only
//! while uniquely owned, by replaying append/index-retirement deltas. Readers
//! of the other buffer remain active during canonical publication and replay.
//! No program, CAS value, edge, ledger, tracker or record is replicated.
//! This module is the replacement seam for a future shared persistent index.
use super::super::queue::{CompactDomain, CompactSummary, Query};
use super::store::{InitialInsertion, Store};
use super::verify::QueryImage;
use std::collections::{BTreeMap, VecDeque};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

pub(super) const LOOKUP_BUFFERS: usize = 2;
pub(super) const MAX_LOOKUP_DELTA_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_LOOKUP_LAG: u64 = 64;

struct Update {
    id: u32,
    retire: Vec<u32>,
}
impl Update {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.retire.len() * std::mem::size_of::<u32>()
    }
    fn prepare(id: u32, retire: &[u32]) -> Result<Self, &'static str> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(retire.len())
            .map_err(|_| "lookup delta retirement allocation")?;
        values.extend_from_slice(retire);
        Ok(Self { id, retire: values })
    }
}

struct Replica<const N: usize> {
    store: Arc<Box<Store<N>>>,
    version: u64,
    applied: u64,
}
struct Views<const N: usize> {
    replicas: Vec<Replica<N>>,
    updates: VecDeque<Update>,
    prepared: VecDeque<Update>,
    first: u64,
    bytes: usize,
}
impl<const N: usize> Views<N> {
    fn new() -> Self {
        Self {
            replicas: Vec::new(),
            updates: VecDeque::new(),
            prepared: VecDeque::new(),
            first: 0,
            bytes: 0,
        }
    }

    fn advance_idle(&mut self, source: &Store<N>, version: u64) -> Result<(), &'static str> {
        for replica in &mut self.replicas {
            let Some(target) = Arc::get_mut(&mut replica.store) else {
                continue;
            };
            if replica.version > version || replica.applied < self.first {
                return Err("lookup replica version/delta range");
            }
            let count = source
                .len()
                .checked_sub(target.len())
                .ok_or("lookup replica watermark ahead")?;
            target.try_reserve(count)?;
            // P3 appends the entire survivor cohort before its index updates.
            // Replay in the same order, with collision-confirmed exact entries.
            for id in target.len()..source.len() {
                let image = source.domains[id];
                let key = image.digest().0;
                target.exact.try_reserve_one(key)?;
                if target
                    .push(image, source.summaries[id], key)
                    .map_err(|_| "lookup replica append differs")?
                    != id as u32
                {
                    return Err("lookup replica ID differs");
                }
            }
            for update in self
                .updates
                .iter()
                .skip((replica.applied - self.first) as usize)
            {
                let image = target.domains[update.id as usize];
                let q = QueryImage::new(image)?;
                let query = Query::new(q.core, image.phase());
                target.index_survivor(update.id, &query, &update.retire)?;
                replica.applied += 1;
            }
            replica.version = version;
        }
        // Exactly two cursors: pruning is proportional to discarded deltas,
        // never to campaign size, bucket count, or a full geometry traversal.
        let first = self
            .replicas
            .iter()
            .map(|r| r.applied)
            .min()
            .unwrap_or(self.first);
        while self.first < first {
            let update = self
                .updates
                .pop_front()
                .ok_or("lookup delta cursor outside journal")?;
            self.bytes -= update.bytes();
            self.first += 1;
        }
        Ok(())
    }

    fn record(&mut self, id: u32, retire: &[u32]) -> Result<(), &'static str> {
        if self.replicas.is_empty() {
            return Ok(());
        }
        let update = if let Some(update) = self.prepared.pop_front() {
            if update.id != id || update.retire != retire {
                return Err("lookup delta differs from preflight");
            }
            update
        } else {
            // Only synthetic initial-admission tests mutate after views exist;
            // native admission finishes before the first lease is constructed.
            self.updates
                .try_reserve(1)
                .map_err(|_| "lookup delta allocation")?;
            Update::prepare(id, retire)?
        };
        if self.bytes.saturating_add(update.bytes()) > MAX_LOOKUP_DELTA_BYTES {
            return Err("lookup delta byte bound");
        }
        self.bytes += update.bytes();
        self.updates.push_back(update);
        Ok(())
    }
}

pub(super) struct StoreOwner<const N: usize> {
    store: Box<Store<N>>,
    views: Mutex<Views<N>>,
}
impl<const N: usize> From<Store<N>> for StoreOwner<N> {
    fn from(store: Store<N>) -> Self {
        Self {
            store: Box::new(store),
            views: Mutex::new(Views::new()),
        }
    }
}
impl<const N: usize> Deref for StoreOwner<N> {
    type Target = Store<N>;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}
impl<const N: usize> StoreOwner<N> {
    pub fn new() -> Self {
        Store::new().into()
    }

    /// The canonical Store is coordinator-owned even while lookup leases live.
    pub fn ensure_unique(&self) -> Result<(), &'static str> {
        Ok(())
    }
    pub fn unique_mut(&mut self) -> Result<&mut Store<N>, &'static str> {
        Ok(&mut self.store)
    }

    pub fn snapshot(&self, version: u64) -> Result<Snapshot<N>, &'static str> {
        self.try_snapshot(version)?
            .ok_or("all lookup buffers still leased")
    }

    pub fn try_snapshot(&self, version: u64) -> Result<Option<Snapshot<N>>, &'static str> {
        u32::try_from(self.len()).map_err(|_| "epoch snapshot watermark exceeds u32")?;
        let mut views = self
            .views
            .lock()
            .map_err(|_| "lookup views lock poisoned")?;
        if views.replicas.is_empty() {
            let mut replicas = Vec::new();
            replicas
                .try_reserve_exact(LOOKUP_BUFFERS)
                .map_err(|_| "lookup replica inventory allocation")?;
            for _ in 0..LOOKUP_BUFFERS {
                replicas.push(Replica {
                    store: Arc::new(Box::new(self.store.try_lookup_clone()?)),
                    version,
                    applied: 0,
                });
            }
            views.replicas = replicas;
        }
        views.advance_idle(&self.store, version)?;
        Ok(views
            .replicas
            .iter()
            .find(|r| r.version == version && r.store.len() == self.len())
            .map(|r| Snapshot {
                version,
                published_len: self.len(),
                store: Arc::clone(&r.store),
            }))
    }

    /// A slow lease never permits an unbounded retirement backlog. The
    /// controller drains results and retries before publishing another cut.
    pub fn publication_room(&self, version: u64, ids: usize, retirements: usize) -> bool {
        let Ok(views) = self.views.lock() else {
            return false;
        };
        if views.replicas.is_empty() {
            return true;
        }
        let added = ids
            .saturating_mul(std::mem::size_of::<Update>())
            .saturating_add(retirements.saturating_mul(std::mem::size_of::<u32>()));
        views.bytes.saturating_add(added) <= MAX_LOOKUP_DELTA_BYTES
            && views
                .replicas
                .iter()
                .all(|r| version.saturating_sub(r.version) < MAX_LOOKUP_LAG)
    }

    /// All journal allocations precede canonical P3 mutation. A failed or
    /// abandoned preflight leaves only replaceable scratch behind.
    pub fn prepare_snapshot_updates<'a>(
        &mut self,
        first: u32,
        retirements: impl ExactSizeIterator<Item = &'a [u32]>,
    ) -> Result<(), &'static str> {
        let views = self
            .views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?;
        views.prepared.clear();
        if views.replicas.is_empty() {
            return Ok(());
        }
        let count = retirements.len();
        views
            .prepared
            .try_reserve(count)
            .map_err(|_| "lookup delta preflight allocation")?;
        views
            .updates
            .try_reserve(count)
            .map_err(|_| "lookup delta allocation")?;
        let mut added = 0usize;
        for (position, retire) in retirements.enumerate() {
            let id = first
                .checked_add(position as u32)
                .ok_or("lookup delta ID overflow")?;
            let update = Update::prepare(id, retire)?;
            added = added
                .checked_add(update.bytes())
                .ok_or("lookup delta size overflow")?;
            if views.bytes.saturating_add(added) > MAX_LOOKUP_DELTA_BYTES {
                return Err("lookup delta byte bound");
            }
            views.prepared.push_back(update);
        }
        Ok(())
    }

    pub fn try_reserve(&mut self, n: usize) -> Result<(), &'static str> {
        self.store.try_reserve(n)
    }
    pub fn reserve_exact(&mut self, digests: &[u64]) -> Result<(), &'static str> {
        self.store.exact.try_reserve(digests)
    }
    pub fn push(
        &mut self,
        image: CompactDomain<N>,
        summary: CompactSummary<N>,
        key: u64,
    ) -> Result<u32, String> {
        self.store.push(image, summary, key)
    }
    pub fn prepare_initial(
        &mut self,
        image: &CompactDomain<N>,
        query: &Query<N>,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<InitialInsertion<N>, &'static str> {
        self.store.prepare_initial(image, query, checkpoint)
    }
    pub fn index_initial(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
        prepared: InitialInsertion<N>,
    ) -> Result<u64, &'static str> {
        let removed = self.store.index_initial(id, query, retire, prepared)?;
        self.views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?
            .record(id, retire)?;
        Ok(removed)
    }
    pub fn index_survivor(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
    ) -> Result<u64, &'static str> {
        let removed = self.store.index_survivor(id, query, retire)?;
        self.views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?
            .record(id, retire)?;
        Ok(removed)
    }

    #[cfg(test)]
    pub fn retained(&self) -> (usize, usize, u64) {
        let views = self.views.lock().unwrap();
        (
            views.replicas.len(),
            views.bytes,
            views.first + views.updates.len() as u64,
        )
    }
}

#[derive(Clone)]
pub(super) struct Snapshot<const N: usize> {
    pub version: u64,
    pub published_len: usize,
    store: Arc<Box<Store<N>>>,
}
impl<const N: usize> Deref for Snapshot<N> {
    type Target = Store<N>;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

/// Legacy single-cut publication plus per-sequence rolling leases. Queued
/// descriptors hold a lease here before enqueue; a worker takes that exact
/// lease once. Refresh can never change a queued job's historical view.
pub(super) struct Publication<const N: usize> {
    slot: Mutex<Option<Snapshot<N>>>,
    jobs: Mutex<BTreeMap<u64, Snapshot<N>>>,
}
impl<const N: usize> Publication<N> {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            jobs: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn publish(&self, snapshot: Snapshot<N>) -> Result<(), &'static str> {
        let mut slot = self
            .slot
            .lock()
            .map_err(|_| "epoch snapshot lock poisoned")?;
        if slot.is_some() {
            return Err("epoch snapshot already published");
        }
        *slot = Some(snapshot);
        Ok(())
    }
    pub fn acquire(&self) -> Result<Snapshot<N>, &'static str> {
        self.slot
            .lock()
            .map_err(|_| "epoch snapshot lock poisoned")?
            .clone()
            .ok_or("epoch lookup snapshot absent")
    }
    pub fn bind(&self, keys: &[u64], snapshot: &Snapshot<N>) -> Result<(), &'static str> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "epoch snapshot jobs lock poisoned")?;
        if keys.len().saturating_add(jobs.len()) > 4096
            || keys
                .iter()
                .enumerate()
                .any(|(at, key)| jobs.contains_key(key) || keys[..at].contains(key))
        {
            return Err("epoch lookup lease inventory/duplicate sequence");
        }
        for &key in keys {
            jobs.insert(key, snapshot.clone());
        }
        Ok(())
    }
    pub fn acquire_job(&self, key: u64) -> Result<Snapshot<N>, &'static str> {
        self.jobs
            .lock()
            .map_err(|_| "epoch snapshot jobs lock poisoned")?
            .remove(&key)
            .ok_or("epoch job lookup lease absent")
    }
    pub fn clear(&self) -> Result<(), &'static str> {
        self.slot
            .lock()
            .map_err(|_| "epoch snapshot lock poisoned")?
            .take();
        self.jobs
            .lock()
            .map_err(|_| "epoch snapshot jobs lock poisoned")?
            .clear();
        Ok(())
    }
}
