//! `EpochState` (W2.0 protocol §3.2): coordinator-owned, the only mutator
//! of persisted walk state. S2 keeps every per-ID array as a plain vector
//! (chunk trees, frozen bits and snapshots are S4/S5 layout work).
use super::super::descendant_closure::Tracker;
use super::anchors::{AnchorMap, MergedView};
use super::edges::EdgeStore;
use super::ledger6::{Entry6, Ledger6, Tag, Transition};
use super::snapshot::StoreOwner;
use super::verify::VerifyCounters;
use std::collections::BTreeMap;

/// `nodes` flags (1 byte per ID). Bit 2 (4) is `closed`, written only into
/// the export from the final forced refresh. `anchored`: the node has an
/// anchor record (any kind); `residual`: a G2' residual record, the same
/// meaning as ledger6's residual bit (an InitialDBand node is anchored and
/// carries ledger6's D-band bit, not `residual`).
pub(super) const NODE_SEALED: u8 = 1;
pub(super) const NODE_INSPECTED: u8 = 2;
pub(super) const NODE_ANCHORED: u8 = 8;
pub(super) const NODE_RESIDUAL: u8 = 16;

/// An in-flight job (dispatched, result not merged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct JobMeta {
    pub seq: u64,
    pub v0: u64,
    /// Runtime lease identity. CP6 reissues unfinished jobs against the
    /// restored current store; historical lookup buffers are not persisted.
    pub published_len: u32,
}

/// Aggregate walk counters (semantics 3 meanings; see the result report).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WalkCounters {
    pub events: u64,
    pub successors: u64,
    pub conditional: u64,
    pub known_reuse: u64,
    pub job_duplicates: u64,
    pub frontiers: u64,
    pub natives: u64,
    pub completed: u64,
    pub native_errors: u64,
    pub routed: u64,
    pub route_masks: u64,
    pub route_joint_pruned: u64,
    pub optional_total: u64,
    pub optional_original: u64,
    pub optional_coalesced: u64,
    pub aliases: u64,
    pub transfers: u64,
    pub retired_lookup_only: u64,
    pub partials: u64,
    pub g2_records: u64,
    pub initial_inspected: u64,
    pub survivors: u64,
    pub antichain_folded: u64,
    pub miss_requests: u64,
    pub dispatched: u64,
    pub discarded: u64,
    pub requeued: u64,
    pub merges: u64,
}

pub(super) struct EpochState<const N: usize> {
    pub store: StoreOwner<N>,
    pub ledger: Ledger6,
    pub nodes: Vec<u8>,
    /// Bit set: the ID is still an entry of the lookup index.
    pub live: Vec<u64>,
    pub edges: EdgeStore,
    pub anchors: AnchorMap,
    pub merged_view: MergedView,
    pub g2_store: Option<std::sync::Arc<super::super::g2::Store<N>>>,
    pub rescue: Option<super::rescue::State>,
    pub tracker: Tracker,
    /// Merge counter k (S_k is the state after merge k).
    pub k: u64,
    /// The protected initial prefix: IDs < P0 never transfer.
    pub p0: u32,
    pub in_flight: BTreeMap<u32, JobMeta>,
    /// Frontier count per NativeFrontier ID (resolution annotations).
    pub frontier_counts: BTreeMap<u32, u32>,
    pub counters: WalkCounters,
    pub verify: VerifyCounters,
    pub lookup: super::store::LookupCounters,
    /// Current-process inspector work accepted into P2 plans (including a later
    /// P3 capacity refusal). Rejected/interrupted cuts are excluded. Not persisted:
    /// restored totals above retain counted operations; this timing is session-only.
    pub inspector_lookup: InspectorLookup,
    pub preparation: super::merge::preparation::Totals,
    pub max_domains: usize,
    pub max_events: u64,
    pub max_frontiers: u64,
    /// Set during P3 or initial admission's authority commit and cleared
    /// only on success (§6.4): a panic or C5 leaves it set, and nothing may
    /// persist a state holding it (the S2 export refuses; S3's save must too).
    pub poisoned: bool,
}

impl<const N: usize> EpochState<N> {
    pub fn new(max_domains: usize, max_events: usize, max_frontiers: usize) -> Self {
        Self {
            store: StoreOwner::new(),
            ledger: Ledger6::default(),
            nodes: Vec::new(),
            live: Vec::new(),
            edges: EdgeStore::new(),
            anchors: AnchorMap::default(),
            merged_view: MergedView::default(),
            g2_store: None,
            rescue: None,
            tracker: Tracker::new(0),
            k: 0,
            p0: 0,
            in_flight: BTreeMap::new(),
            frontier_counts: BTreeMap::new(),
            counters: WalkCounters::default(),
            verify: VerifyCounters::default(),
            lookup: Default::default(),
            inspector_lookup: InspectorLookup::default(),
            preparation: Default::default(),
            max_domains,
            max_events: max_events as u64,
            max_frontiers: max_frontiers as u64,
            poisoned: false,
        }
    }

    /// W_k: the first unassigned ID.
    pub fn watermark(&self) -> u32 {
        self.store.len() as u32
    }

    /// The domain cap F9: `min(max_domains, u32::MAX - 1)`.
    pub fn id_cap(&self) -> usize {
        self.max_domains.min(u32::MAX as usize - 1)
    }

    pub fn is_live(&self, id: u32) -> bool {
        self.live
            .get(id as usize / 64)
            .is_some_and(|word| word >> (id % 64) & 1 == 1)
    }
    pub fn set_live(&mut self, id: u32, live: bool) {
        let word = &mut self.live[id as usize / 64];
        if live {
            *word |= 1 << (id % 64);
        } else {
            *word &= !(1 << (id % 64));
        }
    }

    /// Capacity for `n` new IDs in every per-ID array (P3 preflight).
    pub fn reserve_ids(&mut self, n: usize) -> Result<(), &'static str> {
        self.store.try_reserve(n)?;
        self.ledger.try_reserve(n)?;
        self.nodes
            .try_reserve(n)
            .map_err(|_| "node flag allocation")?;
        let words = (self.store.len() + n).div_ceil(64);
        if let Some(rescue) = &mut self.rescue {
            rescue
                .abandoned
                .try_reserve(words.saturating_sub(rescue.abandoned.len()))
                .map_err(|_| "abandoned bitset allocation")?;
        }
        self.live
            .try_reserve(words.saturating_sub(self.live.len()))
            .map_err(|_| "live bitset allocation")?;
        Ok(())
    }

    /// Per-ID arrays for a freshly pushed ID (after `store.push`): T1,
    /// node flags 0, live.
    pub fn admit_id(&mut self, id: u32) {
        if let Some(rescue) = &mut self.rescue {
            rescue.abandoned.resize(self.store.len().div_ceil(64), 0);
        }
        self.ledger
            .apply(id, Transition::T1New { dispatch_class: 0 })
            .expect("T1 at the watermark after preflight");
        self.nodes.push(0);
        let words = (id as usize + 1).div_ceil(64);
        if self.live.len() < words {
            self.live.resize(words, 0);
        }
        self.set_live(id, true);
    }

    pub fn tag(&self, id: u32) -> Tag {
        self.ledger.tag(id).expect("valid ledger entry")
    }

    pub fn pending_or_reserved(&self) -> u64 {
        let counts = self.ledger.counts();
        counts.get(Tag::Pending) + counts.get(Tag::Reserved)
    }

    pub fn is_sealed(&self, id: u32) -> bool {
        self.nodes[id as usize] & NODE_SEALED != 0
    }

    /// The F8 seal rule on the ledger: sealed iff Native or Alias.
    #[cfg(test)]
    pub fn seal_rule_holds(&self) -> bool {
        (0..self.store.len() as u32).all(|id| {
            let sealed = self.is_sealed(id);
            let entry = self.ledger.get(id).expect("valid ledger entry");
            sealed == matches!(entry, Entry6::Native { .. } | Entry6::Alias { .. })
        })
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub(super) struct InspectorLookup {
    /// Distinct obligations looked up. Timing also includes duplicate validation.
    pub queries: u64,
    pub stored_hits: u64,
    /// Full coordinator Store::lookup calls avoided after a same-view inspector
    /// miss and an independent exact-uniqueness check. Not a count of candidate
    /// tests or nonempty forward-index probes: an absent bucket also qualifies.
    pub coordinator_miss_rechecks_skipped: u64,
    pub seconds: f64,
}
