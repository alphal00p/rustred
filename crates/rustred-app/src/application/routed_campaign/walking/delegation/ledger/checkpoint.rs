//! Persist responsibility, never convert an interrupted native call to completion.
use super::*;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    responsibility: Responsibility,
    initial_anchor: Option<NonZeroUsize>,
    #[serde(default)]
    delegated_published: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub(in super::super::super) struct StoredLedger {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    outstanding_native: Option<usize>,
    entries: Vec<StoredEntry>,
    cursor: usize,
    lookahead: NonZeroUsize,
    reserved_through: usize,
    max_domains: usize,
    transfers: usize,
    native_publications: usize,
    delegated_publications: usize,
    halted: bool,
    initial_admission: bool,
    protected_initial_prefix: Option<usize>,
    partial_initial_inspections: usize,
}
pub(in super::super::super) struct LedgerRef<'a, K>(pub &'a Ledger<K>);
struct Entries<'a, K>(&'a [Entry<K>]);
impl<K> Serialize for Entries<'_, K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for entry in self.0 {
            seq.serialize_element(&StoredEntry {
                responsibility: entry.responsibility,
                initial_anchor: entry.initial_anchor,
                delegated_published: Some(entry.delegated_published),
            })?;
        }
        seq.end()
    }
}
impl<K> Serialize for LedgerRef<'_, K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(bound = "")]
        struct Ref<'a, K> {
            ready: bool,
            // Same shape as StoredLedger: bincode is not self-describing.
            outstanding_native: Option<usize>,
            entries: Entries<'a, K>,
            cursor: usize,
            lookahead: NonZeroUsize,
            reserved_through: usize,
            max_domains: usize,
            transfers: usize,
            native_publications: usize,
            delegated_publications: usize,
            halted: bool,
            initial_admission: bool,
            protected_initial_prefix: Option<usize>,
            partial_initial_inspections: usize,
        }
        let l = self.0;
        Ref {
            ready: l.ready,
            outstanding_native: Some(l.outstanding_native),
            entries: Entries(&l.entries),
            cursor: l.cursor,
            lookahead: l.lookahead,
            reserved_through: l.reserved_through,
            max_domains: l.max_domains,
            transfers: l.transfers,
            native_publications: l.native_publications,
            delegated_publications: l.delegated_publications,
            halted: l.halted,
            initial_admission: l.initial_admission,
            protected_initial_prefix: l.protected_initial_prefix,
            partial_initial_inspections: l.partial_initial_inspections,
        }
        .serialize(serializer)
    }
}
impl StoredLedger {
    pub(in super::super::super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(in super::super::super) fn restore<K: Copy + Eq>(
        self,
        keys: impl ExactSizeIterator<Item = K>,
    ) -> Result<Ledger<K>, String> {
        let len = self.entries.len();
        if keys.len() != len
            || self.cursor > len
            || self.reserved_through > len
            || len > self.max_domains
            || self.halted
            || self.initial_admission
            || self.protected_initial_prefix.is_some_and(|n| n > len)
        {
            return Err("invalid or failed checkpoint responsibility ledger".into());
        }
        let legacy = !self.ready && self.outstanding_native.is_none();
        if !legacy
            && self
                .entries
                .iter()
                .any(|entry| entry.delegated_published.is_none())
        {
            return Err("checkpoint entry has no publication bit".into());
        }
        let entries: Vec<_> = self
            .entries
            .into_iter()
            .zip(keys)
            .enumerate()
            .map(|(id, (e, key))| Entry {
                key,
                responsibility: e.responsibility,
                initial_anchor: e.initial_anchor,
                delegated_published: e.delegated_published.unwrap_or_else(|| {
                    id < self.cursor && matches!(e.responsibility, Responsibility::Delegate { .. })
                }),
            })
            .collect();
        for (id, e) in entries.iter().enumerate() {
            match e.responsibility {
                Responsibility::Delegate { to }
                    if to <= id || to >= len || entries[to].key != e.key =>
                {
                    return Err("invalid checkpoint delegation edge".into());
                }
                Responsibility::Local(Local::Published(
                    NativeOutcome::Failed | NativeOutcome::Cancelled,
                )) => return Err("checkpoint contains failed native publication".into()),
                Responsibility::Local(Local::Published(_)) if !self.ready && id >= self.cursor => {
                    return Err("checkpoint publication beyond cursor".into());
                }
                Responsibility::Local(Local::Unreserved | Local::Reserved) if id < self.cursor => {
                    return Err("checkpoint incomplete obligation before cursor".into());
                }
                _ => {}
            }
            if let Some(anchor) = e.initial_anchor {
                let anchor = anchor.get() - 1;
                if anchor >= id
                    || self.protected_initial_prefix.is_none_or(|n| anchor >= n)
                    || entries[anchor].key != e.key
                {
                    return Err("invalid checkpoint initial anchor".into());
                }
            }
        }
        // CP1 did not encode either field. Only its original Ordered state is
        // reconstructed; Ready always requires its explicit counter and bits.
        let outstanding_native = match self.outstanding_native {
            Some(n) => n,
            None if !self.ready => entries
                .iter()
                .filter(|e| {
                    matches!(
                        e.responsibility,
                        Responsibility::Local(Local::Reserved | Local::Started)
                    )
                })
                .count(),
            None => return Err("ready checkpoint has no outstanding-native count".into()),
        };
        let mut ledger = Ledger {
            ready: self.ready,
            outstanding_native,
            entries,
            cursor: self.cursor,
            lookahead: self.lookahead,
            reserved_through: self.reserved_through,
            max_domains: self.max_domains,
            transfers: self.transfers,
            native_publications: self.native_publications,
            delegated_publications: self.delegated_publications,
            halted: false,
            initial_admission: false,
            protected_initial_prefix: self.protected_initial_prefix,
            partial_initial_inspections: self.partial_initial_inspections,
        };
        ledger.restore_normalize_started()?;
        Ok(ledger)
    }
}
