//! Persist responsibility, never convert an interrupted native call to completion.
use super::*;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    responsibility: Responsibility,
    initial_anchor: Option<NonZeroUsize>,
}

#[derive(Serialize, Deserialize)]
pub(in super::super::super) struct StoredLedger {
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
        let entries: Vec<_> = self
            .entries
            .into_iter()
            .zip(keys)
            .map(|(e, key)| Entry {
                key,
                responsibility: match e.responsibility {
                    Responsibility::Local(Local::Started) => Responsibility::Local(Local::Reserved),
                    other => other,
                },
                initial_anchor: e.initial_anchor,
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
                Responsibility::Local(Local::Published(_)) if id >= self.cursor => {
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
        Ok(Ledger {
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
        })
    }
}
