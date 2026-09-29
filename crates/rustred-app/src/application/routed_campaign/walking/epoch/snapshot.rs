//! Private lockstep lookup snapshots. There is one store, not a copy per
//! inspector. Readers release their Arc before returning result bytes; the
//! coordinator clears publication after the cut drains and then checks unique
//! ownership. Interrupted saves may still borrow the immutable store before join.
use super::super::queue::{CompactDomain, CompactSummary, Query};
use super::store::{InitialInsertion, Store};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

pub(super) struct StoreOwner<const N: usize>(Arc<Box<Store<N>>>);

impl<const N: usize> From<Store<N>> for StoreOwner<N> {
    fn from(store: Store<N>) -> Self {
        Self(Arc::new(Box::new(store)))
    }
}

impl<const N: usize> Deref for StoreOwner<N> {
    type Target = Store<N>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> StoreOwner<N> {
    pub fn new() -> Self {
        Store::new().into()
    }

    pub fn ensure_unique(&self) -> Result<(), &'static str> {
        if Arc::strong_count(&self.0) == 1 && Arc::weak_count(&self.0) == 0 {
            Ok(())
        } else {
            Err("epoch lookup snapshot still shared at mutation boundary")
        }
    }

    pub fn unique_mut(&mut self) -> Result<&mut Store<N>, &'static str> {
        Arc::get_mut(&mut self.0)
            .map(Box::as_mut)
            .ok_or("epoch lookup snapshot still shared at mutation boundary")
    }

    pub fn snapshot(&self, version: u64) -> Result<Snapshot<N>, &'static str> {
        u32::try_from(self.len()).map_err(|_| "epoch snapshot watermark exceeds u32")?;
        Ok(Snapshot {
            version,
            published_len: self.len(),
            store: Arc::clone(&self.0),
        })
    }

    pub fn try_reserve(&mut self, n: usize) -> Result<(), &'static str> {
        self.unique_mut()?.try_reserve(n)
    }

    pub fn reserve_exact(&mut self, digests: &[u64]) -> Result<(), &'static str> {
        self.unique_mut()?.exact.try_reserve(digests)
    }

    pub fn push(
        &mut self,
        image: CompactDomain<N>,
        summary: CompactSummary<N>,
        key: u64,
    ) -> Result<u32, String> {
        self.unique_mut()?.push(image, summary, key)
    }

    pub fn prepare_initial(
        &mut self,
        image: &CompactDomain<N>,
        query: &Query<N>,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<InitialInsertion<N>, &'static str> {
        self.unique_mut()?.prepare_initial(image, query, checkpoint)
    }

    pub fn index_initial(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
        prepared: InitialInsertion<N>,
    ) -> Result<u64, &'static str> {
        self.unique_mut()?
            .index_initial(id, query, retire, prepared)
    }

    pub fn index_survivor(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
    ) -> Result<u64, &'static str> {
        self.unique_mut()?.index_survivor(id, query, retire)
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

/// One lock per job, never per successor. Queued jobs contain only bytes and
/// completed results cannot retain a snapshot. No refresh within a lockstep cut.
pub(super) struct Publication<const N: usize>(Mutex<Option<Snapshot<N>>>);

impl<const N: usize> Publication<N> {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn publish(&self, snapshot: Snapshot<N>) -> Result<(), &'static str> {
        let mut slot = self.0.lock().map_err(|_| "epoch snapshot lock poisoned")?;
        if slot.is_some() {
            return Err("epoch snapshot already published");
        }
        *slot = Some(snapshot);
        Ok(())
    }

    pub fn acquire(&self) -> Result<Snapshot<N>, &'static str> {
        self.0
            .lock()
            .map_err(|_| "epoch snapshot lock poisoned")?
            .clone()
            .ok_or("epoch lookup snapshot absent")
    }

    pub fn clear(&self) -> Result<(), &'static str> {
        self.0
            .lock()
            .map_err(|_| "epoch snapshot lock poisoned")?
            .take();
        Ok(())
    }
}
