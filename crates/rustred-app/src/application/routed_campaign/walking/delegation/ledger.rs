use super::types::{Error, NativeOutcome, Publication, Transfer};
use std::num::NonZeroUsize;
mod checkpoint;
#[cfg(test)]
mod ready_tests;
pub(in super::super) use checkpoint::{LedgerRef, StoredLedger};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Local {
    Unreserved,
    Reserved,
    Started,
    Published(NativeOutcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Responsibility {
    Local(Local),
    Delegate { to: usize },
}

#[derive(Clone, Debug)]
pub(super) struct Entry<K> {
    pub key: K,
    pub responsibility: Responsibility,
    pub initial_anchor: Option<NonZeroUsize>,
    /// Alias publication is explicit: Ready may publish beyond the watermark.
    /// Native entries always leave this false.
    pub delegated_published: bool,
}

/// One queue-local immutable snapshot. K is the exact phase/owner key, normally
/// (Phase, [bool; N]); no hashing, Symbolica expressions or geometry clones.
/// The scheduler retains streamed/escrowed native results outside this ledger.
#[derive(Debug)]
pub struct Ledger<K> {
    pub(super) entries: Vec<Entry<K>>,
    pub(super) cursor: usize,
    lookahead: NonZeroUsize,
    reserved_through: usize,
    ready: bool,
    outstanding_native: usize,
    max_domains: usize,
    pub(super) transfers: usize,
    pub(super) native_publications: usize,
    pub(super) delegated_publications: usize,
    halted: bool,
    initial_admission: bool,
    pub(super) protected_initial_prefix: Option<usize>,
    partial_initial_inspections: usize,
}

impl<K: Copy + Eq> Ledger<K> {
    pub fn new(lookahead: NonZeroUsize, max_domains: usize) -> Result<Self, Error> {
        if max_domains == 0 {
            return Err(Error::ZeroCapacity);
        }
        Ok(Self {
            entries: Vec::new(),
            cursor: 0,
            lookahead,
            reserved_through: 0,
            ready: false,
            outstanding_native: 0,
            max_domains,
            transfers: 0,
            native_publications: 0,
            delegated_publications: 0,
            halted: false,
            initial_admission: false,
            protected_initial_prefix: None,
            partial_initial_inspections: 0,
        })
    }

    /// Ready uses outstanding native-parent credits, not a cursor-relative
    /// numeric interval. Native publication holes release one credit each.
    pub fn new_ready(lookahead: NonZeroUsize, max_domains: usize) -> Result<Self, Error> {
        let mut ledger = Self::new(lookahead, max_domains)?;
        ledger.ready = true;
        Ok(ledger)
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn reservation_scan(&self) -> usize {
        self.reserved_through
    }

    pub fn transfers(&self) -> usize {
        self.transfers
    }

    pub fn outstanding_native(&self) -> usize {
        self.outstanding_native
    }

    pub fn published_count(&self) -> usize {
        // Disjoint publications, each performed at most once for an admitted ID.
        self.native_publications + self.delegated_publications
    }

    pub fn is_published(&self, id: usize) -> bool {
        self.entries
            .get(id)
            .is_some_and(|entry| match entry.responsibility {
                Responsibility::Local(Local::Published(_)) => true,
                Responsibility::Delegate { .. } => entry.delegated_published,
                _ => false,
            })
    }

    /// Startup/resume inventory. This scans once without allocating; callers
    /// retain at most H reserved IDs, then follow the monotone reservation scan.
    pub fn ready_reserved_ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().enumerate().filter_map(|(id, entry)| {
            (entry.responsibility == Responsibility::Local(Local::Reserved)).then_some(id)
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn lookahead(&self) -> NonZeroUsize {
        self.lookahead
    }

    pub fn dispatch_fence(&self) -> usize {
        if self.ready {
            self.reserved_through
        } else {
            self.cursor.saturating_add(self.lookahead.get())
        }
    }

    pub fn native_publications(&self) -> usize {
        self.native_publications
    }

    pub fn delegated_publications(&self) -> usize {
        self.delegated_publications
    }

    pub fn transfer_count(&self) -> usize {
        self.transfers
    }

    /// Begin BEFORE the first initial admission; pinning after retirement would
    /// be too late when the initial prefix exceeds the dispatch horizon.
    pub fn begin_initial_admission(&mut self) -> Result<(), Error> {
        if !self.entries.is_empty()
            || self.protected_initial_prefix.is_some()
            || self.initial_admission
        {
            return Err(Error::InvalidInitialPhase);
        }
        self.initial_admission = true;
        Ok(())
    }

    pub fn finish_initial_admission(&mut self) -> Result<(), Error> {
        if !self.initial_admission {
            return Err(Error::InvalidInitialPhase);
        }
        self.initial_admission = false;
        self.protected_initial_prefix = Some(self.entries.len());
        Ok(())
    }

    pub fn initial_prefix(&self) -> Option<usize> {
        self.protected_initial_prefix
    }

    pub fn partial_initial_inspections(&self) -> usize {
        self.partial_initial_inspections
    }

    /// No allocation: link storage was reserved with the ordinary admission.
    /// Geometry is established by the same-snapshot native split planner.
    pub fn record_initial_overlap(&mut self, id: usize, anchor: usize) -> Result<(), Error> {
        self.check_publisher(id)?;
        let prefix = self
            .protected_initial_prefix
            .ok_or(Error::InvalidInitialAnchor)?;
        if id < prefix || anchor >= prefix || anchor >= id {
            return Err(Error::InvalidInitialAnchor);
        }
        let source = &self.entries[anchor];
        if source.key != self.entries[id].key
            || source.initial_anchor.is_some()
            || matches!(source.responsibility, Responsibility::Delegate { .. })
        {
            return Err(Error::InvalidInitialAnchor);
        }
        let entry = &mut self.entries[id];
        if entry.responsibility != Responsibility::Local(Local::Started)
            || entry.initial_anchor.is_some()
        {
            return Err(Error::InvalidNativeState);
        }
        entry.initial_anchor =
            NonZeroUsize::new(anchor.checked_add(1).ok_or(Error::InvalidInitialAnchor)?);
        self.partial_initial_inspections += 1;
        Ok(())
    }

    /// Capacity-only part of the queue's admission preflight. No logical state
    /// changes on failure. Call before the queue retires ANY native candidate.
    /// Geometric growth is explicitly capped by max_domains.
    pub fn reserve_admission(&mut self, id: usize) -> Result<(), Error> {
        if self.halted {
            return Err(Error::Halted);
        }
        if id != self.entries.len() {
            return Err(Error::AdmissionIdMismatch);
        }
        if id >= self.max_domains {
            return Err(Error::Capacity);
        }
        if self.entries.len() == self.entries.capacity() {
            let wanted = self
                .entries
                .capacity()
                .saturating_mul(2)
                .max(16)
                .min(self.max_domains);
            self.entries
                .try_reserve_exact(wanted - self.entries.len())
                .map_err(|_| Error::Allocation)?;
        }
        Ok(())
    }

    /// Commit immediately before the queue's already-preflighted, infallible
    /// exact retirement/index/domain publication block. No allocation occurs.
    /// The outer coordinator transaction must publish its raw domain atomically
    /// with this entry; do not expose it to any observer partway through.
    pub fn admit_reserved(&mut self, id: usize, key: K) -> Result<(), Error> {
        if self.halted {
            return Err(Error::Halted);
        }
        if id != self.entries.len() {
            return Err(Error::AdmissionIdMismatch);
        }
        if id >= self.max_domains {
            return Err(Error::Capacity);
        }
        if self.entries.len() == self.entries.capacity() {
            return Err(Error::AdmissionNotReserved);
        }
        self.entries.push(Entry {
            key,
            responsibility: Responsibility::Local(Local::Unreserved),
            initial_anchor: None,
            delegated_published: false,
        });
        self.reserve_horizon();
        Ok(())
    }

    /// Called ONLY by the queue's exact reverse-retirement callback when the
    /// new domain contains old under the same immutable native snapshot.
    /// This service validates identity/state, not mathematical containment.
    ///
    /// Invalid/protected proposals are refused without mutation. The queue may
    /// still retire their lookup entry: the original native obligation remains.
    /// Thus a refusal cannot erase responsibility or fail after partial commit.
    pub fn transfer_retired(&mut self, old: usize, representative: usize) -> Transfer {
        if representative.checked_add(1) != Some(self.entries.len()) || old >= representative {
            return Transfer::InvalidForwardEdge;
        }
        if self.entries[old].key != self.entries[representative].key {
            return Transfer::IdentityMismatch;
        }
        if self.initial_admission
            || self
                .protected_initial_prefix
                .is_some_and(|prefix| old < prefix)
        {
            return Transfer::ProtectedInitial;
        }
        if matches!(
            self.entries[old].responsibility,
            Responsibility::Delegate { .. }
        ) {
            return Transfer::AlreadyDelegated;
        }
        if self.halted
            || (!self.ready && old < self.dispatch_fence())
            || self.entries[old].responsibility != Responsibility::Local(Local::Unreserved)
        {
            return Transfer::ReservedOrStarted;
        }
        self.entries[old].responsibility = Responsibility::Delegate { to: representative };
        // Each admitted ID can be assigned once, so transfers <= entries.len().
        self.transfers += 1;
        Transfer::Installed
    }

    pub fn delegated_to(&self, id: usize) -> Option<usize> {
        match self.entries.get(id)?.responsibility {
            Responsibility::Delegate { to } => Some(to),
            Responsibility::Local(_) => None,
        }
    }

    pub fn can_dispatch(&self, id: usize) -> bool {
        !self.halted
            && (self.ready || id < self.dispatch_fence())
            && self
                .entries
                .get(id)
                .is_some_and(|entry| entry.responsibility == Responsibility::Local(Local::Reserved))
    }

    /// Mark only an actual successful pool dispatch or an inline native start.
    /// A reserved ID is protected even before a physical worker is assigned.
    pub fn native_started(&mut self, id: usize) -> Result<(), Error> {
        if self.halted {
            return Err(Error::Halted);
        }
        if !self.ready && id >= self.dispatch_fence() {
            return Err(Error::OutsideFence);
        }
        let entry = self.entries.get_mut(id).ok_or(Error::InvalidId)?;
        if entry.responsibility != Responsibility::Local(Local::Reserved) {
            return Err(Error::InvalidNativeState);
        }
        entry.responsibility = Responsibility::Local(Local::Started);
        Ok(())
    }

    /// No native Finished/statistics are fabricated. The caller must pre-reserve
    /// its typed report record before advancing this canonical cursor.
    pub fn publish_delegated(&mut self, id: usize) -> Result<Publication, Error> {
        self.check_publisher(id)?;
        let Responsibility::Delegate { to } = self.entries[id].responsibility else {
            return Err(Error::NotDelegated);
        };
        self.entries[id].delegated_published = true;
        self.delegated_publications += 1;
        self.advance_watermark();
        self.reserve_horizon();
        Ok(Publication::DelegatedNotInspected {
            id,
            representative: to,
        })
    }

    /// Call only after every native event/successor is canonically committed.
    /// Capture the native frontier count BEFORE the scheduler moves its details.
    /// A finished-but-escrowed job remains Started here until actual publication.
    pub fn publish_native(
        &mut self,
        id: usize,
        outcome: NativeOutcome,
    ) -> Result<Publication, Error> {
        self.check_publisher(id)?;
        if self.entries[id].responsibility != Responsibility::Local(Local::Started) {
            return Err(Error::InvalidNativeState);
        }
        let outstanding = self
            .outstanding_native
            .checked_sub(1)
            .ok_or(Error::InvalidNativeState)?;
        self.entries[id].responsibility = Responsibility::Local(Local::Published(outcome));
        self.native_publications += 1;
        self.outstanding_native = outstanding;
        self.halted = matches!(outcome, NativeOutcome::Failed | NativeOutcome::Cancelled);
        self.advance_watermark();
        self.reserve_horizon();
        Ok(Publication::Native { id, outcome })
    }

    fn check_publisher(&self, id: usize) -> Result<(), Error> {
        if self.halted {
            return Err(Error::Halted);
        }
        if !self.ready && id != self.cursor {
            return Err(Error::NotCurrentPublisher);
        }
        if id >= self.entries.len() {
            return Err(Error::InvalidId);
        }
        if self.is_published(id) {
            return Err(Error::InvalidNativeState);
        }
        Ok(())
    }

    fn reserve_horizon(&mut self) {
        if self.ready {
            self.reserve_available();
            return;
        }
        let end = self.dispatch_fence().min(self.entries.len());
        for id in self.reserved_through..end {
            if self.entries[id].responsibility == Responsibility::Local(Local::Unreserved) {
                self.entries[id].responsibility = Responsibility::Local(Local::Reserved);
                self.outstanding_native += 1;
            }
        }
        // Aliases remain sticky as this monotone fence advances.
        self.reserved_through = end;
    }

    /// Coordinator-only, allocation-free credit refill. Each ID is scanned at
    /// most once, including aliases; completed holes never retain a credit.
    pub fn reserve_available(&mut self) {
        if !self.ready {
            self.reserve_horizon();
            return;
        }
        if self.halted {
            return;
        }
        while self.outstanding_native < self.lookahead.get()
            && self.reserved_through < self.entries.len()
        {
            let entry = &mut self.entries[self.reserved_through];
            if entry.responsibility == Responsibility::Local(Local::Unreserved) {
                entry.responsibility = Responsibility::Local(Local::Reserved);
                self.outstanding_native += 1;
            }
            self.reserved_through += 1;
        }
    }

    fn advance_watermark(&mut self) {
        while self.is_published(self.cursor) {
            self.cursor += 1;
        }
    }

    /// Validate the complete persisted responsibility image without repairing
    /// malformed counters or inferring published holes from a numeric cursor.
    /// This intentionally scans once at a checkpoint boundary, never per event.
    pub fn validate_checkpoint(&self) -> Result<(), String> {
        let len = self.entries.len();
        if self.max_domains == 0
            || len > self.max_domains
            || self.cursor > len
            || self.reserved_through > len
            || self.halted
            || self.initial_admission
            || self.protected_initial_prefix.is_some_and(|n| n > len)
        {
            return Err("invalid or failed checkpoint responsibility ledger".into());
        }
        let mut native = 0;
        let mut delegated = 0;
        let mut transfers = 0;
        let mut pending = 0;
        let mut partial = 0;
        let mut watermark = len;
        let ordered_end = self.cursor.saturating_add(self.lookahead.get()).min(len);
        for (id, entry) in self.entries.iter().enumerate() {
            let published = self.is_published(id);
            if !published && watermark == len {
                watermark = id;
            }
            if !self.ready && published != (id < self.cursor) {
                return Err("checkpoint ordered publication is not a contiguous prefix".into());
            }
            match entry.responsibility {
                Responsibility::Delegate { to } => {
                    transfers += 1;
                    delegated += usize::from(entry.delegated_published);
                    if to <= id || to >= len || self.entries[to].key != entry.key {
                        return Err("invalid checkpoint delegation edge".into());
                    }
                    if self.protected_initial_prefix.is_some_and(|n| id < n) {
                        return Err("checkpoint delegates a protected initial obligation".into());
                    }
                    if entry.initial_anchor.is_some() {
                        return Err("checkpoint delegated obligation has an initial anchor".into());
                    }
                }
                Responsibility::Local(local) => {
                    if entry.delegated_published {
                        return Err("checkpoint native obligation has an alias publication".into());
                    }
                    match local {
                        Local::Published(NativeOutcome::Failed | NativeOutcome::Cancelled) => {
                            return Err("checkpoint contains failed native publication".into());
                        }
                        Local::Published(_) => native += 1,
                        Local::Reserved | Local::Started => pending += 1,
                        Local::Unreserved => {}
                    }
                    let scan = if self.ready {
                        self.reserved_through
                    } else {
                        ordered_end
                    };
                    if matches!(local, Local::Unreserved) != (id >= scan) {
                        return Err("checkpoint native reservation scan is inconsistent".into());
                    }
                }
            }
            if let Some(anchor) = entry.initial_anchor {
                partial += 1;
                let anchor = anchor.get() - 1;
                let prefix = self
                    .protected_initial_prefix
                    .ok_or("checkpoint initial anchor lacks prefix")?;
                if id < prefix || anchor >= prefix || anchor >= id {
                    return Err("invalid checkpoint initial anchor ID".into());
                }
                let source = &self.entries[anchor];
                if source.key != entry.key
                    || source.initial_anchor.is_some()
                    || matches!(source.responsibility, Responsibility::Delegate { .. })
                    || matches!(
                        entry.responsibility,
                        Responsibility::Local(Local::Unreserved)
                    )
                {
                    return Err("invalid checkpoint initial anchor responsibility".into());
                }
            }
        }
        if watermark != self.cursor
            || native != self.native_publications
            || delegated != self.delegated_publications
            || transfers != self.transfers
            || pending != self.outstanding_native
            || partial != self.partial_initial_inspections
            || pending > self.lookahead.get()
            || (!self.ready && self.reserved_through != ordered_end)
            || (self.ready && pending < self.lookahead.get() && self.reserved_through != len)
        {
            return Err(
                "checkpoint responsibility counters, watermark or credits are inconsistent".into(),
            );
        }
        Ok(())
    }

    /// A clean interruption is unfinished work, never a fabricated Finished.
    /// Preserve reservations, aliases, completed holes and every anchor link.
    pub fn restore_normalize_started(&mut self) -> Result<(), String> {
        self.validate_checkpoint()?;
        for entry in &mut self.entries {
            if entry.responsibility == Responsibility::Local(Local::Started) {
                entry.responsibility = Responsibility::Local(Local::Reserved);
            }
        }
        Ok(())
    }
}
