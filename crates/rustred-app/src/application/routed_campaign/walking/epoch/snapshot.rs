//! Shared immutable lookup roots, independent of coordinator mutation.
//!
//! Geometry uses copy-on-write pages; indexes use bounded geometric cohorts.
//! Canonical P3 remains the only owner of the mutable Store. Readers pin an
//! exact root; publication prepares a complete replacement off to the side.
//! No CAS value, edge, ledger, tracker or record is replicated.
use super::super::queue::{CompactDomain, CompactSummary, Query};
use super::store::{InitialInsertion, Store};
use std::collections::{BTreeMap, VecDeque};
use std::ops::Deref;
use std::sync::{Arc, Mutex, Weak};

mod image;
pub(super) mod shared;

pub(super) const MAX_LOOKUP_DELTA_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_LOOKUP_LAG: u64 = 64;
pub(super) const MAX_LOOKUP_ROOTS: usize = 64;

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

struct Root<const N: usize> {
    version: u64,
    image: Arc<image::Image<N>>,
    charge: u64,
}
struct Historical<const N: usize> {
    version: u64,
    image: Weak<image::Image<N>>,
    charge: u64,
}
struct Views<const N: usize> {
    current: Option<Root<N>>,
    historical: VecDeque<Historical<N>>,
    updates: VecDeque<Update>,
    prepared: VecDeque<Update>,
    bytes: usize,
}
impl<const N: usize> Views<N> {
    fn new() -> Self {
        Self {
            current: None,
            historical: VecDeque::new(),
            updates: VecDeque::new(),
            prepared: VecDeque::new(),
            bytes: 0,
        }
    }
    fn prune(&mut self) {
        self.historical
            .retain(|root| root.image.strong_count() != 0);
    }
    fn readers(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|root| Arc::strong_count(&root.image) > 1)
            || self
                .historical
                .iter()
                .any(|root| root.image.strong_count() != 0)
    }
    fn room(&self, version: u64, canonical_len: usize, ids: usize, retirements: usize) -> bool {
        let Some(current) = &self.current else {
            return true;
        };
        if !self.readers() {
            return true;
        }
        let historical = self
            .historical
            .iter()
            .filter(|r| r.image.strong_count() != 0);
        let mut count = 0;
        let mut oldest_charge = current.charge;
        for root in historical {
            count += 1;
            oldest_charge = oldest_charge.min(root.charge);
            if version.saturating_sub(root.version) >= MAX_LOOKUP_LAG {
                return false;
            }
        }
        if Arc::strong_count(&current.image) > 1 {
            count += 1;
            if version.saturating_sub(current.version) >= MAX_LOOKUP_LAG {
                return false;
            }
        }
        let additions = canonical_len
            .saturating_sub(current.image.len())
            .saturating_add(ids);
        let retirements = self
            .updates
            .iter()
            .fold(retirements, |n, r| n.saturating_add(r.retire.len()));
        let charge = current.image.refresh_charge(additions, retirements) as u64;
        count < MAX_LOOKUP_ROOTS
            && current
                .charge
                .saturating_sub(oldest_charge)
                .saturating_add(charge)
                <= MAX_LOOKUP_DELTA_BYTES as u64
    }
    fn record(&mut self, id: u32, retire: &[u32]) -> Result<(), &'static str> {
        if self.current.is_none() {
            return Ok(());
        }
        let update = if let Some(update) = self.prepared.pop_front() {
            if update.id != id || update.retire != retire {
                return Err("lookup delta differs from preflight");
            }
            update
        } else {
            // Initial-admission synthetic tests can append after bootstrap.
            // Native P3 always reserves all updates in its preflight.
            self.updates
                .try_reserve(1)
                .map_err(|_| "lookup delta allocation")?;
            Update::prepare(id, retire)?
        };
        self.bytes = self
            .bytes
            .checked_add(update.bytes())
            .ok_or("lookup delta size overflow")?;
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
    pub fn ensure_unique(&self) -> Result<(), &'static str> {
        Ok(())
    }
    pub fn unique_mut(&mut self) -> Result<&mut Store<N>, &'static str> {
        Ok(&mut self.store)
    }

    pub fn enable_rescue_duplicates(&mut self) -> Result<(), &'static str> {
        let views = self
            .views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?;
        if views.readers() {
            return Err("rescue lookup change while readers are active");
        }
        *views = Views::new();
        self.store.rescue_duplicates = true;
        Ok(())
    }
    pub fn install_quarantine(&mut self, bits: Vec<u64>) -> Result<(), &'static str> {
        let views = self
            .views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?;
        if views.readers() {
            return Err("rescue lookup change while readers are active");
        }
        self.store.install_quarantine(bits)?;
        *views = Views::new();
        Ok(())
    }
    pub fn snapshot(&self, version: u64) -> Result<Snapshot<N>, &'static str> {
        self.try_snapshot(version)?
            .ok_or("lookup retained-root bound requires reader drain")
    }
    pub fn try_snapshot(&self, version: u64) -> Result<Option<Snapshot<N>>, &'static str> {
        self.try_snapshot_with(version, &mut || Ok(()))
    }
    pub fn try_snapshot_with(
        &self,
        version: u64,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Option<Snapshot<N>>, &'static str> {
        u32::try_from(self.len()).map_err(|_| "epoch snapshot watermark exceeds u32")?;
        let mut views = self
            .views
            .lock()
            .map_err(|_| "lookup views lock poisoned")?;
        views.prune();
        checkpoint()?;
        if let Some(current) = &views.current {
            if current.version > version || current.image.len() > self.len() {
                return Err("lookup snapshot version/watermark ahead");
            }
            if current.image.len() == self.len() {
                if !views.updates.is_empty() {
                    return Err("lookup unchanged root has pending deltas");
                }
                let image = Arc::clone(&current.image);
                // Same geometry, not a stale lookup root. Historical leases
                // retain their own reported version; no data is overwritten.
                views.current.as_mut().expect("current root").version = version;
                return Ok(Some(Snapshot {
                    version,
                    published_len: self.len(),
                    image,
                }));
            }
        }
        if !views.room(version, self.len(), 0, 0) {
            return Ok(None);
        }
        views
            .historical
            .try_reserve(1)
            .map_err(|_| "lookup root inventory allocation")?;
        let mut copies = shared::Copies::default();
        let (candidate, charge) = if let Some(current) = &views.current {
            let added = self.len() - current.image.len();
            let retired = views.updates.iter().map(|u| u.retire.len()).sum();
            let charge = current
                .charge
                .checked_add(current.image.refresh_charge(added, retired) as u64)
                .ok_or("lookup retention charge overflow")?;
            let candidate = current.image.advance(
                &self.store,
                views.updates.iter().map(|u| (u.id, u.retire.as_slice())),
                &mut copies,
                checkpoint,
            )?;
            (candidate, charge)
        } else {
            (
                image::Image::bootstrap(&self.store, &mut copies, checkpoint)?,
                0,
            )
        };
        checkpoint()?;
        let image = Arc::new(candidate);
        let old = views.current.replace(Root {
            version,
            image: Arc::clone(&image),
            charge,
        });
        if let Some(old) = old {
            if Arc::strong_count(&old.image) > 1 {
                views.historical.push_back(Historical {
                    version: old.version,
                    image: Arc::downgrade(&old.image),
                    charge: old.charge,
                });
            }
        }
        views.updates.clear();
        views.bytes = 0;
        Ok(Some(Snapshot {
            version,
            published_len: self.len(),
            image,
        }))
    }

    pub fn publication_room(&self, version: u64, ids: usize, retirements: usize) -> bool {
        self.views
            .lock()
            .is_ok_and(|views| views.room(version, self.len(), ids, retirements))
    }
    pub fn prepare_snapshot_updates<'a>(
        &mut self,
        first: u32,
        digests: &[u64],
        retirements: impl ExactSizeIterator<Item = &'a [u32]> + Clone,
    ) -> Result<(), &'static str> {
        self.prepare_updates_with_bound(first, digests, retirements, MAX_LOOKUP_DELTA_BYTES)
    }
    fn prepare_updates_with_bound<'a>(
        &mut self,
        first: u32,
        digests: &[u64],
        retirements: impl ExactSizeIterator<Item = &'a [u32]> + Clone,
        limit: usize,
    ) -> Result<(), &'static str> {
        let views = self
            .views
            .get_mut()
            .map_err(|_| "lookup views lock poisoned")?;
        views.prepared.clear();
        if views.current.is_none() {
            return Ok(());
        }
        let count = retirements.len();
        if count != digests.len() || first as usize != self.store.len() {
            return Err("lookup preflight digest/retirement cardinality or watermark");
        }
        let bytes = retirements
            .clone()
            .try_fold(
                count.saturating_mul(std::mem::size_of::<Update>()),
                |sum, retire| {
                    sum.checked_add(retire.len().saturating_mul(std::mem::size_of::<u32>()))
                },
            )
            .ok_or("lookup delta size overflow")?;
        // Oversized legal cuts may run at a quiescent boundary. They are not
        // silently truncated and cannot accumulate behind historical readers.
        if views.bytes.saturating_add(bytes) > limit && views.readers() {
            return Err("lookup publication requires reader drain");
        }
        views
            .prepared
            .try_reserve(count)
            .map_err(|_| "lookup delta preflight allocation")?;
        views
            .updates
            .try_reserve(count)
            .map_err(|_| "lookup delta allocation")?;
        for (position, retire) in retirements.enumerate() {
            let position = u32::try_from(position).map_err(|_| "lookup delta ID range")?;
            let id = first
                .checked_add(position)
                .ok_or("lookup delta ID overflow")?;
            views.prepared.push_back(Update::prepare(id, retire)?);
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
    pub fn prepare_direct_for_test<'a>(
        &mut self,
        first: u32,
        digests: &[u64],
        retirements: impl ExactSizeIterator<Item = &'a [u32]> + Clone,
    ) -> Result<(), &'static str> {
        self.prepare_updates_with_bound(first, digests, retirements, 0)
    }
    #[cfg(test)]
    pub fn retained(&self) -> (usize, usize, u64) {
        let views = self.views.lock().unwrap();
        (
            usize::from(views.current.is_some())
                + views
                    .historical
                    .iter()
                    .filter(|r| r.image.strong_count() != 0)
                    .count(),
            views.bytes,
            views.current.as_ref().map_or(0, |r| r.version),
        )
    }
}

#[derive(Clone)]
pub(super) struct Snapshot<const N: usize> {
    pub version: u64,
    pub published_len: usize,
    image: Arc<image::Image<N>>,
}
impl<const N: usize> Deref for Snapshot<N> {
    type Target = image::Image<N>;
    fn deref(&self) -> &Self::Target {
        &self.image
    }
}
#[cfg(test)]
impl<const N: usize> Snapshot<N> {
    pub fn same_root(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.image, &other.image)
    }
}

/// Legacy single-cut publication plus per-sequence rolling leases. Queued
/// descriptors hold a lease here before enqueue; a worker takes that exact
/// lease once. Refresh can never change a queued job's historical view.
///
/// Only the real native Snapshot controller binds a session. Synthetic
/// callbacks and decoded result bytes cannot opt into negative-prefix reuse.
pub(super) struct Publication<const N: usize> {
    slot: Mutex<Option<Snapshot<N>>>,
    jobs: Mutex<BTreeMap<u64, Snapshot<N>>>,
    native_session: Option<NativeSession>,
}

/// Invocation-local routing context, not a proof of a returned negative.
/// Rescue runs before this context exists; replay uses a new durable session.
/// This marker is never serialized or reconstructed from a LookupReport.
#[derive(Clone, Copy)]
pub(super) struct NativeSession {
    session: u64,
}

impl NativeSession {
    pub fn matches(self, seq: u64) -> bool {
        self.session != 0 && seq >> 40 == self.session
    }
}

impl<const N: usize> Publication<N> {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            jobs: Mutex::new(BTreeMap::new()),
            native_session: None,
        }
    }

    /// Called only by run_native_observed in Snapshot mode, after any
    /// resume-boundary amendments and replay sequence reassignment.
    pub fn native(session: u64) -> Self {
        Self {
            native_session: Some(NativeSession { session }),
            ..Self::new()
        }
    }

    pub fn native_session(&self) -> Option<NativeSession> {
        self.native_session
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
