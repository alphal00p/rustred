//! The pending frontier of the Ordered scheduler.
//!
//! `Classic` (default) is the historical layout: a hash map from key to
//! pending weight plus an ordered set of `(phase, priority, key)` clones.
//!
//! `phased` is the default of p requests (`OrderedProfile::P`, M5 batch 2);
//! `RUSTRED_ORDERED_FRONTIER=classic|compact|phased` (rr-frontier) overrides
//! the profile. `compact` or `phased` selects
//! `Packed`: every pending key is stored ONCE, packed (`FrontierKey`: small
//! integer powers inline, no heap allocation in the common case), in a slot
//! of a slab together with its certified position and weight. A hash table
//! of slot ids indexes the keys (one table for all phases, so a contribution
//! to a key of any phase merges, and cancels, on insert exactly as in
//! `Classic`), and every phase has its own bucket of slot ids ordered by
//! `(priority, key)`:
//!
//! - `compact`: every bucket is a binary heap at all times;
//! - `phased` (the analogue of FORM's spectators): a bucket of a later phase
//!   is an append-only list of ids, heapified once when its phase starts, so
//!   keys that the current phase cannot touch are neither sorted nor moved.
//!
//! Both pop exactly the keys `Classic` pops, in the same order: within a
//! phase the greatest `(priority, key)` first (the packed comparison is the
//! key type's own `Ord`), cancelled keys stay behind as tombstones that are
//! discarded when reached and revived if the key returns first, as the
//! classic order entries are. Values, guards and counters are unchanged.

use std::borrow::Cow;
use std::cmp::{Ordering, Reverse};
use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap};
use std::hash::{BuildHasher, Hash};

use hashbrown::HashTable;

use super::{FrontierHash, OrderedPosition, Pending, Scheduled};
use crate::family::IntegralKey;
use crate::reduction::ReductionError;

/// A scheduled key type and its packed form inside a `Packed` frontier.
/// `pack` must be injective (equal packed forms exactly for equal keys) and
/// `cmp_packed` must order packed forms as `Ord` orders the keys.
pub(crate) trait FrontierKey: Clone + Ord + Hash {
    type Packed: Clone + Eq + Hash + Send + Sync;
    fn pack(&self) -> Self::Packed;
    fn unpack(packed: &Self::Packed) -> Self;
    fn cmp_packed(a: &Self::Packed, b: &Self::Packed) -> Ordering;
    /// rr-frontier diagnostics: allocator bytes a packed key owns.
    fn packed_heap_bytes(_packed: &Self::Packed) -> usize {
        0
    }
}

/// Keys stored as they are.
macro_rules! identity_frontier_key {
    ($($t:ty),*) => {$(
        impl FrontierKey for $t {
            type Packed = $t;
            fn pack(&self) -> $t {
                self.clone()
            }
            fn unpack(packed: &$t) -> $t {
                packed.clone()
            }
            fn cmp_packed(a: &$t, b: &$t) -> Ordering {
                a.cmp(b)
            }
        }
    )*};
}
identity_frontier_key!(usize, i64, u64);

/// Inline capacity of [`CompactPowers`]: covers the 5-loop families, also
/// as p-library views (20 powers + the 16 dense Gamma-tag slots = 36; the
/// 4-loop views have 14 + 16 = 30).
pub(crate) const INLINE_POWERS: usize = 38;
const TAG_BIT: u8 = 0x80;

/// Integer powers packed in 40 bytes: up to [`INLINE_POWERS`] powers in
/// `i8` inline, otherwise (a longer key or a power outside `i8`) the wide
/// form on the heap. The form is canonical (inline whenever it fits), so the
/// derived equality and hash are those of the power sequence and the tag.
/// One caller bit (`tag`) rides along in the metadata byte.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CompactPowers {
    Inline {
        /// Bit 7: tag; bits 0-6: number of powers.
        meta: u8,
        powers: [i8; INLINE_POWERS],
    },
    Wide {
        /// Bit 7: tag.
        meta: u8,
        powers: Box<[i64]>,
    },
}

impl CompactPowers {
    pub(crate) fn new(powers: &[i64], tag: bool) -> Self {
        let tag = if tag { TAG_BIT } else { 0 };
        if powers.len() <= INLINE_POWERS && powers.iter().all(|&p| i8::try_from(p).is_ok()) {
            let mut inline = [0_i8; INLINE_POWERS];
            for (slot, &power) in inline.iter_mut().zip(powers) {
                *slot = power as i8;
            }
            Self::Inline {
                meta: powers.len() as u8 | tag,
                powers: inline,
            }
        } else {
            Self::Wide {
                meta: tag,
                powers: powers.into(),
            }
        }
    }

    pub(crate) fn tag(&self) -> bool {
        match self {
            Self::Inline { meta, .. } | Self::Wide { meta, .. } => meta & TAG_BIT != 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Inline { meta, .. } => usize::from(meta & !TAG_BIT),
            Self::Wide { powers, .. } => powers.len(),
        }
    }

    fn get(&self, index: usize) -> i64 {
        match self {
            Self::Inline { powers, .. } => i64::from(powers[index]),
            Self::Wide { powers, .. } => powers[index],
        }
    }

    pub(crate) fn to_vec(&self) -> Vec<i64> {
        match self {
            Self::Inline { powers, .. } => {
                powers[..self.len()].iter().map(|&p| i64::from(p)).collect()
            }
            Self::Wide { powers, .. } => powers.to_vec(),
        }
    }

    /// The order of `Vec<i64>` (lexicographic, then length); tags ignored.
    pub(crate) fn cmp_powers(&self, other: &Self) -> Ordering {
        if let (Self::Inline { powers: a, .. }, Self::Inline { powers: b, .. }) = (self, other) {
            let (la, lb) = (self.len(), other.len());
            let n = la.min(lb);
            return a[..n].cmp(&b[..n]).then(la.cmp(&lb));
        }
        let (la, lb) = (self.len(), other.len());
        for i in 0..la.min(lb) {
            match self.get(i).cmp(&other.get(i)) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        la.cmp(&lb)
    }

    /// rr-alloc: the same powers with the caller bit set to `tag`.
    pub(crate) fn with_tag(&self, tag: bool) -> Self {
        let mut packed = self.clone();
        let meta = match &mut packed {
            Self::Inline { meta, .. } | Self::Wide { meta, .. } => meta,
        };
        *meta = if tag {
            *meta | TAG_BIT
        } else {
            *meta & !TAG_BIT
        };
        packed
    }

    /// rr-alloc: the powers appended to `out` (cleared first).
    pub(crate) fn decode_into<A: smallvec::Array<Item = i64>>(
        &self,
        out: &mut smallvec::SmallVec<A>,
    ) {
        out.clear();
        match self {
            Self::Inline { powers, .. } => {
                out.extend(powers[..self.len()].iter().map(|&p| i64::from(p)));
            }
            Self::Wide { powers, .. } => out.extend_from_slice(powers),
        }
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        match self {
            Self::Inline { .. } => 0,
            Self::Wide { powers, .. } => super::memstats::block(powers.as_ptr(), powers.len()),
        }
    }
}

impl FrontierKey for IntegralKey {
    type Packed = CompactPowers;
    fn pack(&self) -> CompactPowers {
        CompactPowers::new(self.powers(), false)
    }
    fn unpack(packed: &CompactPowers) -> IntegralKey {
        IntegralKey::try_from_preallocated(packed.to_vec()).expect("a packed key is nonempty")
    }
    fn cmp_packed(a: &CompactPowers, b: &CompactPowers) -> Ordering {
        a.cmp_powers(b)
    }
    fn packed_heap_bytes(packed: &CompactPowers) -> usize {
        packed.heap_bytes()
    }
}

/// Which frontier layout a sequential run uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrontierMode {
    Classic,
    Compact,
    Phased,
}

impl FrontierMode {
    /// `RUSTRED_ORDERED_FRONTIER=classic|compact|phased` overrides the
    /// request's profile (`OrderedProfile`); unset or another value: `None`.
    pub(crate) fn environment() -> Option<Self> {
        static MODE: std::sync::OnceLock<Option<FrontierMode>> = std::sync::OnceLock::new();
        *MODE.get_or_init(
            || match std::env::var("RUSTRED_ORDERED_FRONTIER").as_deref() {
                Ok("classic") => Some(Self::Classic),
                Ok("compact") => Some(Self::Compact),
                Ok("phased") => Some(Self::Phased),
                _ => None,
            },
        )
    }
}

/// The layout and hash of one request's frontier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrontierLayout {
    pub mode: FrontierMode,
    pub fx_hash: bool,
}

impl From<FrontierMode> for FrontierLayout {
    fn from(mode: FrontierMode) -> Self {
        Self {
            mode,
            fx_hash: false,
        }
    }
}

/// The historical layout (see the module docs).
pub(crate) struct Classic<K, P, W> {
    pub pending: HashMap<K, Pending<P, W>, FrontierHash>,
    pub order: BTreeSet<Scheduled<K, P>>,
}

#[derive(Clone)]
struct Slot<C, W> {
    key: C,
    /// Interned priority (see `Priorities`).
    priority: u32,
    phase: u32,
    /// `None`: a tombstone (cancelled or removed, its bucket entry pending).
    weight: Option<W>,
}

#[derive(Clone, Default)]
struct Bucket {
    ids: Vec<u32>,
    /// `ids` is a max-heap by `(priority, key)`.
    heap: bool,
}

/// Interned priorities of a `Packed` frontier, reference counted by slots:
/// many pending keys share one priority (owner rank and order key), and a
/// slot then holds 4 bytes instead of the value. Interning is by `Eq` (and
/// `Hash`); comparisons stay `Ord` on the values, so nothing is reordered.
#[derive(Clone)]
struct Priorities<P> {
    values: Vec<Option<(P, u32)>>,
    free: Vec<u32>,
    index: HashTable<u32>,
}

impl<P: Clone + Ord + Hash> Priorities<P> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            free: Vec::new(),
            index: HashTable::new(),
        }
    }

    #[inline]
    fn get(&self, id: u32) -> &P {
        &self.values[id as usize].as_ref().expect("live priority").0
    }

    #[inline]
    fn cmp(&self, a: u32, b: u32) -> Ordering {
        if a == b {
            Ordering::Equal
        } else {
            self.get(a).cmp(self.get(b))
        }
    }

    fn intern(&mut self, hasher: &FrontierHash, priority: P) -> u32 {
        let hash = hasher.hash_one(&priority);
        let values = &self.values;
        if let Some(&id) = self.index.find(hash, |&id| {
            values[id as usize]
                .as_ref()
                .is_some_and(|(p, _)| *p == priority)
        }) {
            self.values[id as usize].as_mut().expect("live priority").1 += 1;
            return id;
        }
        let id = match self.free.pop() {
            Some(id) => {
                self.values[id as usize] = Some((priority, 1));
                id
            }
            None => {
                self.values.push(Some((priority, 1)));
                (self.values.len() - 1) as u32
            }
        };
        let values = &self.values;
        self.index.insert_unique(hash, id, |&x| {
            hasher.hash_one(&values[x as usize].as_ref().expect("live priority").0)
        });
        id
    }

    fn release(&mut self, hasher: &FrontierHash, id: u32) {
        let entry = self.values[id as usize].as_mut().expect("live priority");
        entry.1 -= 1;
        if entry.1 == 0 {
            let hash = hasher.hash_one(&entry.0);
            if let Ok(found) = self.index.find_entry(hash, |&x| x == id) {
                found.remove();
            }
            self.values[id as usize] = None;
            self.free.push(id);
        }
    }

    fn live(&self) -> usize {
        self.values.len() - self.free.len()
    }
}

/// The packed layout (see the module docs).
pub(crate) struct Packed<K: FrontierKey, P, W> {
    slots: Vec<Option<Slot<K::Packed, W>>>,
    priorities: Priorities<P>,
    free: Vec<u32>,
    index: HashTable<u32>,
    hasher: FrontierHash,
    live: usize,
    buckets: Vec<Bucket>,
    /// `phased`: buckets of later phases are appended to, not heaped.
    lazy: bool,
    /// Diagnostics: merges and cancellations into keys of a later phase
    /// than the popped key (what a frontier without a hash over later
    /// phases would defer to their phase start).
    pub later_merges: usize,
    pub later_cancels: usize,
    pub current: usize,
}

pub(crate) enum Frontier<K: FrontierKey, P, W> {
    Classic(Classic<K, P, W>),
    Packed(Packed<K, P, W>),
}

/// What a child contribution finds.
pub(crate) enum Child<'f, K, P, W> {
    Occupied {
        phase: usize,
        priority: &'f P,
        weight: &'f mut W,
    },
    Vacant(&'f K),
}

/// What the caller does with it.
pub(crate) enum ChildAction<P, W> {
    Keep,
    Remove,
    Insert(Pending<P, W>),
}

fn sift_up<T>(v: &mut [T], mut i: usize, cmp: &impl Fn(&T, &T) -> Ordering) {
    while i > 0 {
        let parent = (i - 1) / 2;
        if cmp(&v[i], &v[parent]) != Ordering::Greater {
            break;
        }
        v.swap(i, parent);
        i = parent;
    }
}

fn sift_down<T>(v: &mut [T], mut i: usize, cmp: &impl Fn(&T, &T) -> Ordering) {
    let n = v.len();
    loop {
        let (l, r) = (2 * i + 1, 2 * i + 2);
        let mut largest = i;
        if l < n && cmp(&v[l], &v[largest]) == Ordering::Greater {
            largest = l;
        }
        if r < n && cmp(&v[r], &v[largest]) == Ordering::Greater {
            largest = r;
        }
        if largest == i {
            return;
        }
        v.swap(i, largest);
        i = largest;
    }
}

impl<K: FrontierKey, P: Clone + Ord + Hash, W> Packed<K, P, W> {
    fn new(lazy: bool, hasher: FrontierHash) -> Self {
        Self {
            slots: Vec::new(),
            priorities: Priorities::new(),
            free: Vec::new(),
            index: HashTable::new(),
            hasher,
            live: 0,
            buckets: Vec::new(),
            lazy,
            later_merges: 0,
            later_cancels: 0,
            current: 0,
        }
    }

    fn slot(&self, id: u32) -> &Slot<K::Packed, W> {
        self.slots[id as usize]
            .as_ref()
            .expect("a bucket entry refers to a live slot")
    }

    #[inline]
    fn cmp_ids(
        slots: &[Option<Slot<K::Packed, W>>],
        priorities: &Priorities<P>,
        a: u32,
        b: u32,
    ) -> Ordering {
        let (a, b) = (
            slots[a as usize].as_ref().expect("live slot"),
            slots[b as usize].as_ref().expect("live slot"),
        );
        priorities
            .cmp(a.priority, b.priority)
            .then_with(|| K::cmp_packed(&a.key, &b.key))
    }

    fn hash(&self, packed: &K::Packed) -> u64 {
        self.hasher.hash_one(packed)
    }

    fn find(&self, hash: u64, packed: &K::Packed) -> Option<u32> {
        let slots = &self.slots;
        self.index
            .find(hash, |&id| {
                slots[id as usize]
                    .as_ref()
                    .is_some_and(|slot| slot.key == *packed)
            })
            .copied()
    }

    fn unindex(&mut self, id: u32) {
        let hash = self.hash(&self.slot(id).key);
        if let Ok(entry) = self.index.find_entry(hash, |&x| x == id) {
            entry.remove();
        }
    }

    fn release(&mut self, id: u32) {
        if let Some(slot) = self.slots[id as usize].take() {
            self.priorities.release(&self.hasher, slot.priority);
        }
        self.free.push(id);
    }

    fn schedule(&mut self, id: u32, phase: usize) {
        let lazy = self.lazy && phase > self.current;
        if self.buckets.len() <= phase {
            self.buckets.resize_with(phase + 1, Bucket::default);
        }
        let Self {
            slots,
            priorities,
            buckets,
            ..
        } = self;
        let bucket = &mut buckets[phase];
        bucket.ids.push(id);
        if lazy {
            bucket.heap = false;
        } else if bucket.heap || bucket.ids.len() == 1 {
            bucket.heap = true;
            let last = bucket.ids.len() - 1;
            sift_up(&mut bucket.ids, last, &|&a, &b| {
                Self::cmp_ids(slots, priorities, a, b)
            });
        } else {
            Self::heapify(slots, priorities, bucket);
        }
    }

    fn heapify(
        slots: &[Option<Slot<K::Packed, W>>],
        priorities: &Priorities<P>,
        bucket: &mut Bucket,
    ) {
        let n = bucket.ids.len();
        for i in (0..n / 2).rev() {
            sift_down(&mut bucket.ids, i, &|&a, &b| {
                Self::cmp_ids(slots, priorities, a, b)
            });
        }
        bucket.heap = true;
    }

    /// A new slot (or a revived tombstone) for `packed` at `position`.
    fn insert(&mut self, hash: u64, packed: K::Packed, pending: Pending<P, W>) {
        let phase = pending.position.phase;
        if let Some(id) = self.find(hash, &packed) {
            let slot = self.slots[id as usize].as_mut().expect("indexed slot");
            debug_assert!(slot.weight.is_none(), "insert into a live key");
            if slot.phase as usize == phase
                && *self.priorities.get(slot.priority) == pending.position.priority
            {
                // The key returned before its bucket entry was reached.
                slot.weight = Some(pending.weight);
                self.live += 1;
                return;
            }
            // A different position (never produced by a pure position
            // function): the old entry becomes an orphan, popped as a
            // tombstone.
            self.unindex(id);
        }
        let priority = self
            .priorities
            .intern(&self.hasher, pending.position.priority);
        let slot = Slot {
            key: packed,
            priority,
            phase: phase as u32,
            weight: Some(pending.weight),
        };
        let id = match self.free.pop() {
            Some(id) => {
                self.slots[id as usize] = Some(slot);
                id
            }
            None => {
                self.slots.push(Some(slot));
                (self.slots.len() - 1) as u32
            }
        };
        let slots = &self.slots;
        let hasher = &self.hasher;
        self.index.insert_unique(hash, id, |&x| {
            hasher.hash_one(&slots[x as usize].as_ref().expect("indexed slot").key)
        });
        self.live += 1;
        self.schedule(id, phase);
    }

    fn pop_in_phase(&mut self, phase: usize) -> Option<(Scheduled<K, P>, W)> {
        self.current = self.current.max(phase);
        if phase >= self.buckets.len() {
            return None;
        }
        if !self.buckets[phase].heap {
            let Self {
                slots,
                priorities,
                buckets,
                ..
            } = self;
            Self::heapify(slots, priorities, &mut buckets[phase]);
        }
        loop {
            let id = {
                let Self {
                    slots,
                    priorities,
                    buckets,
                    ..
                } = self;
                let bucket = &mut buckets[phase];
                let last = bucket.ids.len().checked_sub(1)?;
                bucket.ids.swap(0, last);
                let id = bucket.ids.pop().expect("nonempty bucket");
                sift_down(&mut bucket.ids, 0, &|&a, &b| {
                    Self::cmp_ids(slots, priorities, a, b)
                });
                id
            };
            self.unindex(id);
            let slot = self.slots[id as usize].take().expect("live slot");
            self.free.push(id);
            let priority = self.priorities.get(slot.priority).clone();
            self.priorities.release(&self.hasher, slot.priority);
            if let Some(weight) = slot.weight {
                self.live -= 1;
                return Some((
                    Scheduled {
                        phase: Reverse(phase),
                        priority,
                        key: K::unpack(&slot.key),
                    },
                    weight,
                ));
            }
        }
    }

    fn remove(&mut self, key: &K) -> Option<W> {
        let packed = key.pack();
        let hash = self.hash(&packed);
        let id = self.find(hash, &packed)?;
        let slot = self.slots[id as usize].as_mut().expect("indexed slot");
        let weight = slot.weight.take()?;
        self.live -= 1;
        Some(weight)
    }

    fn clear_order(&mut self) {
        let buckets = std::mem::take(&mut self.buckets);
        for id in buckets.into_iter().flat_map(|bucket| bucket.ids) {
            if self.slot(id).weight.is_none() {
                self.unindex(id);
                self.release(id);
            }
        }
    }

    fn earlier_than(&self, phase: usize) -> bool {
        self.buckets
            .iter()
            .take(phase)
            .any(|bucket| !bucket.ids.is_empty())
    }

    /// Bucket `phase` in pop order (live and tombstone ids).
    fn ordered_ids(&self, phase: usize) -> Vec<u32> {
        let bucket = &self.buckets[phase];
        let (slots, priorities) = (&self.slots, &self.priorities);
        let mut ids = bucket.ids.clone();
        ids.sort_unstable_by(|&a, &b| Self::cmp_ids(slots, priorities, b, a));
        ids
    }

    fn iter(&self) -> impl Iterator<Item = (Cow<'_, Scheduled<K, P>>, &W)> + '_ {
        (0..self.buckets.len()).flat_map(move |phase| {
            let ids: Box<dyn Iterator<Item = u32> + '_> = if self.buckets[phase].heap {
                Box::new(HeapWalk::<K, P, W>::new(
                    &self.buckets[phase].ids,
                    &self.slots,
                    &self.priorities,
                ))
            } else {
                Box::new(self.ordered_ids(phase).into_iter())
            };
            ids.filter_map(move |id| {
                let slot = self.slot(id);
                let weight = slot.weight.as_ref()?;
                Some((
                    Cow::Owned(Scheduled {
                        phase: Reverse(phase),
                        priority: self.priorities.get(slot.priority).clone(),
                        key: K::unpack(&slot.key),
                    }),
                    weight,
                ))
            })
        })
    }

    pub(crate) fn memstats(&self) -> serde_json::Value
    where
        P: Clone,
        W: Clone,
    {
        use super::memstats::{block, measure_clone};
        let key_heap: usize = (self.slots.iter().flatten())
            .map(|slot| K::packed_heap_bytes(&slot.key))
            .sum();
        let wide = (self.slots.iter().flatten())
            .filter(|slot| K::packed_heap_bytes(&slot.key) > 0)
            .count();
        let tombstones = (self.slots.iter().flatten())
            .filter(|slot| slot.weight.is_none())
            .count();
        let buckets: usize = self
            .buckets
            .iter()
            .map(|b| block(b.ids.as_ptr(), b.ids.capacity()))
            .sum();
        serde_json::json!({
            "priorities": self.priorities.live(),
            "priorities_clone_bytes": measure_clone(|| self.priorities.clone()),
            "slot_size": std::mem::size_of::<Option<Slot<K::Packed, W>>>(),
            "packed_key_size": std::mem::size_of::<K::Packed>(),
            "slots": self.slots.len(),
            "slots_capacity": self.slots.capacity(),
            "slots_bytes": block(self.slots.as_ptr(), self.slots.capacity()),
            "slots_clone_bytes": measure_clone(|| self.slots.clone()),
            "tombstones": tombstones,
            "free": self.free.len(),
            "free_bytes": block(self.free.as_ptr(), self.free.capacity()),
            "index_capacity": self.index.capacity(),
            "index_clone_bytes": measure_clone(|| self.index.clone()),
            "bucket_bytes": buckets,
            "wide_keys": wide,
            "key_heap_bytes": key_heap,
            "later_merges": self.later_merges,
            "later_cancels": self.later_cancels,
        })
    }

    /// Priority heap bytes and weights of the live slots (diagnostics).
    pub(crate) fn live(&self) -> impl Iterator<Item = (&P, &W)> {
        (self.slots.iter().flatten())
            .filter_map(|slot| Some((self.priorities.get(slot.priority), slot.weight.as_ref()?)))
    }
}

/// The ids of a heap in pop order, lazily (an auxiliary heap of positions).
struct HeapWalk<'h, K: FrontierKey, P, W> {
    heap: &'h [u32],
    slots: &'h [Option<Slot<K::Packed, W>>],
    priorities: &'h Priorities<P>,
    aux: Vec<usize>,
}

impl<'h, K: FrontierKey, P: Clone + Ord + Hash, W> HeapWalk<'h, K, P, W> {
    fn new(
        heap: &'h [u32],
        slots: &'h [Option<Slot<K::Packed, W>>],
        priorities: &'h Priorities<P>,
    ) -> Self {
        Self {
            heap,
            slots,
            priorities,
            aux: if heap.is_empty() { vec![] } else { vec![0] },
        }
    }
}

impl<'h, K: FrontierKey, P: Clone + Ord + Hash, W> HeapWalk<'h, K, P, W> {
    /// rr-par2: the walk of the subtrees under `roots` (heap positions none
    /// of which is an ancestor of another), in pop order.
    fn from_roots(
        heap: &'h [u32],
        slots: &'h [Option<Slot<K::Packed, W>>],
        priorities: &'h Priorities<P>,
        mut roots: Vec<usize>,
    ) -> Self {
        let cmp = |&a: &usize, &b: &usize| {
            Packed::<K, P, W>::cmp_ids(slots, priorities, heap[a], heap[b])
        };
        for i in (0..roots.len() / 2).rev() {
            sift_down(&mut roots, i, &cmp);
        }
        Self {
            heap,
            slots,
            priorities,
            aux: roots,
        }
    }
}

impl<K: FrontierKey, P: Clone + Ord + Hash, W> Iterator for HeapWalk<'_, K, P, W> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        let (heap, slots, priorities) = (self.heap, self.slots, self.priorities);
        let cmp = |&a: &usize, &b: &usize| {
            Packed::<K, P, W>::cmp_ids(slots, priorities, heap[a], heap[b])
        };
        let last = self.aux.len().checked_sub(1)?;
        self.aux.swap(0, last);
        let top = self.aux.pop().expect("nonempty walk");
        sift_down(&mut self.aux, 0, &cmp);
        for child in [2 * top + 1, 2 * top + 2] {
            if child < heap.len() {
                self.aux.push(child);
                let at = self.aux.len() - 1;
                sift_up(&mut self.aux, at, &cmp);
            }
        }
        Some(heap[top])
    }
}

impl<K: FrontierKey, P: Clone + Ord + Hash, W> Frontier<K, P, W> {
    pub(crate) fn new(layout: impl Into<FrontierLayout>) -> Self {
        let layout = layout.into();
        let hasher = FrontierHash::new(layout.fx_hash);
        match layout.mode {
            FrontierMode::Classic => Self::Classic(Classic {
                pending: HashMap::with_hasher(hasher),
                order: BTreeSet::new(),
            }),
            FrontierMode::Compact => Self::Packed(Packed::new(false, hasher)),
            FrontierMode::Phased => Self::Packed(Packed::new(true, hasher)),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Classic(c) => c.pending.len(),
            Self::Packed(p) => p.live,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The classic layout (the concurrent exact scheduler works on it).
    pub(crate) fn classic_mut(&mut self) -> Result<&mut Classic<K, P, W>, ReductionError> {
        match self {
            Self::Classic(c) => Ok(c),
            Self::Packed(_) => Err(ReductionError::ReducerInvariant {
                detail: "the concurrent ordered scheduler needs the classic frontier",
            }),
        }
    }

    /// Pending entries in pop order. A cancelled key leaves its order entry
    /// behind until it is reached; such entries are skipped.
    #[allow(clippy::type_complexity)] // one boxed iterator over both layouts
    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, Scheduled<K, P>>, &W)> + '_> {
        match self {
            Self::Classic(c) => Box::new(c.order.iter().rev().filter_map(|scheduled| {
                let pending = c.pending.get(&scheduled.key)?;
                (pending.position.phase == scheduled.phase.0
                    && pending.position.priority == scheduled.priority)
                    .then_some((Cow::Borrowed(scheduled), &pending.weight))
            })),
            Self::Packed(p) => Box::new(p.iter()),
        }
    }

    /// Pop the greatest pending key of `phase`, discarding cancelled entries.
    pub(crate) fn pop_in_phase(&mut self, phase: usize) -> Option<(Scheduled<K, P>, W)> {
        match self {
            Self::Classic(c) => {
                while c
                    .order
                    .last()
                    .is_some_and(|scheduled| scheduled.phase.0 == phase)
                {
                    let scheduled = c.order.pop_last().expect("nonempty checked frontier");
                    if let Some(pending) = c.pending.remove(&scheduled.key) {
                        return Some((scheduled, pending.weight));
                    }
                }
                None
            }
            Self::Packed(p) => p.pop_in_phase(phase),
        }
    }

    /// Whether an order entry of a phase before `phase` exists.
    pub(crate) fn earlier_than(&self, phase: usize) -> bool {
        match self {
            Self::Classic(c) => c.order.last().is_some_and(|node| node.phase.0 < phase),
            Self::Packed(p) => p.earlier_than(phase),
        }
    }

    /// Admit a key that is not pending.
    pub(crate) fn insert(&mut self, key: K, pending: Pending<P, W>) {
        match self {
            Self::Classic(c) => {
                c.order.insert(Scheduled {
                    phase: Reverse(pending.position.phase),
                    priority: pending.position.priority.clone(),
                    key: key.clone(),
                });
                c.pending.insert(key, pending);
            }
            Self::Packed(p) => {
                let packed = key.pack();
                let hash = p.hash(&packed);
                p.insert(hash, packed, pending);
            }
        }
    }

    /// Remove a pending key (its order entry stays behind): its weight.
    pub(crate) fn remove(&mut self, key: &K) -> Option<W> {
        match self {
            Self::Classic(c) => c.pending.remove(key).map(|pending| pending.weight),
            Self::Packed(p) => p.remove(key),
        }
    }

    /// Drop every order entry (closing tail).
    pub(crate) fn clear_order(&mut self) {
        match self {
            Self::Classic(c) => c.order.clear(),
            Self::Packed(p) => p.clear_order(),
        }
    }

    /// rr-par2: whether a key packed and hashed elsewhere with the Fx frontier
    /// hash ([`FrontierHash::Fx`]) can be probed here ([`Self::child_packed`]):
    /// a packed frontier with the Fx hash.
    pub(crate) fn takes_prepacked(&self) -> bool {
        matches!(self, Self::Packed(p) if matches!(p.hasher, FrontierHash::Fx))
    }

    /// fwd-opt: the first phase after `after` with pending keys and its
    /// greatest `n` pending keys with their positions (in no particular
    /// order; ties and order are left to the caller's sort).
    #[allow(clippy::type_complexity)]
    pub(crate) fn next_phase_top(
        &self,
        after: usize,
        n: usize,
    ) -> Option<(usize, Vec<(K, OrderedPosition<P>)>)> {
        if n == 0 {
            return None;
        }
        let p = match self {
            Self::Classic(_) => {
                let mut phase = None;
                let mut out = Vec::new();
                for (scheduled, _) in self.iter() {
                    let q = scheduled.phase.0;
                    if q <= after {
                        continue;
                    }
                    match phase {
                        None => phase = Some(q),
                        Some(p) if p != q => break,
                        Some(_) => {}
                    }
                    out.push((
                        scheduled.key.clone(),
                        OrderedPosition {
                            phase: q,
                            priority: scheduled.priority.clone(),
                        },
                    ));
                    if out.len() == n {
                        break;
                    }
                }
                return phase.map(|phase| (phase, out));
            }
            Self::Packed(p) => p,
        };
        for q in after + 1..p.buckets.len() {
            let mut ids: Vec<u32> = p.buckets[q]
                .ids
                .iter()
                .copied()
                .filter(|&id| p.slot(id).weight.is_some())
                .collect();
            if ids.is_empty() {
                continue;
            }
            if ids.len() > n {
                let (slots, priorities) = (&p.slots, &p.priorities);
                ids.select_nth_unstable_by(n - 1, |&a, &b| {
                    Packed::<K, P, W>::cmp_ids(slots, priorities, b, a)
                });
                ids.truncate(n);
            }
            let out = ids
                .into_iter()
                .map(|id| {
                    let slot = p.slot(id);
                    (
                        K::unpack(&slot.key),
                        OrderedPosition {
                            phase: q,
                            priority: p.priorities.get(slot.priority).clone(),
                        },
                    )
                })
                .collect();
            return Some((q, out));
        }
        None
    }

    /// rr-par2: the first `n` pending keys of `phase` below `threshold`, in pop
    /// order, with their positions: what
    /// `iter().take_while(phase).skip_while(>= threshold).take(n)` yields, without
    /// materialising the skipped entries and without ordering a later phase.
    pub(crate) fn phase_candidates(
        &self,
        phase: usize,
        threshold: &Scheduled<K, P>,
        n: usize,
    ) -> Vec<(K, OrderedPosition<P>)> {
        let position = |scheduled: &Scheduled<K, P>| OrderedPosition {
            phase,
            priority: scheduled.priority.clone(),
        };
        let p = match self {
            Self::Classic(_) => {
                return self
                    .iter()
                    .take_while(|(scheduled, _)| scheduled.phase.0 == phase)
                    .skip_while(|(scheduled, _)| **scheduled >= *threshold)
                    .take(n)
                    .map(|(scheduled, _)| (scheduled.key.clone(), position(&scheduled)))
                    .collect();
            }
            Self::Packed(p) => p,
        };
        let mut out = Vec::new();
        if phase >= p.buckets.len() || n == 0 || threshold.phase.0 != phase {
            return out;
        }
        let bound = threshold.key.pack();
        let at_or_above = |id: u32| {
            let slot = p.slot(id);
            p.priorities
                .get(slot.priority)
                .cmp(&threshold.priority)
                .then_with(|| K::cmp_packed(&slot.key, &bound))
                != Ordering::Less
        };
        let ids: Box<dyn Iterator<Item = u32> + '_> = if p.buckets[phase].heap {
            // The entries at or above the threshold form a subtree at the
            // top of the heap: find the positions just below it (one
            // comparison per visited entry), then walk best-first from them.
            let heap = &p.buckets[phase].ids;
            let mut stack = if heap.is_empty() {
                vec![]
            } else {
                vec![0usize]
            };
            let mut roots = Vec::new();
            while let Some(at) = stack.pop() {
                if at_or_above(heap[at]) {
                    stack.extend(
                        [2 * at + 1, 2 * at + 2]
                            .into_iter()
                            .filter(|&c| c < heap.len()),
                    );
                } else {
                    roots.push(at);
                }
            }
            Box::new(HeapWalk::<K, P, W>::from_roots(
                heap,
                &p.slots,
                &p.priorities,
                roots,
            ))
        } else {
            Box::new(p.ordered_ids(phase).into_iter())
        };
        let mut skipping = !p.buckets[phase].heap;
        for id in ids {
            let slot = p.slot(id);
            if slot.weight.is_none() {
                continue;
            }
            let priority = p.priorities.get(slot.priority);
            if skipping {
                if at_or_above(id) {
                    continue;
                }
                skipping = false;
            }
            out.push((
                K::unpack(&slot.key),
                OrderedPosition {
                    phase,
                    priority: priority.clone(),
                },
            ));
            if out.len() == n {
                break;
            }
        }
        out
    }

    /// rr-par2: [`Self::child`] for a key already packed and hashed (Fx) by a
    /// worker. Only for frontiers that [`Self::takes_prepacked`].
    pub(crate) fn child_packed<E>(
        &mut self,
        key: K,
        packed: K::Packed,
        hash: u64,
        f: impl FnOnce(Child<'_, K, P, W>) -> Result<ChildAction<P, W>, E>,
    ) -> Result<Option<W>, E> {
        let Self::Packed(p) = self else {
            return self.child(key, f);
        };
        debug_assert_eq!(
            hash,
            p.hash(&packed),
            "a prepacked key carries its frontier hash"
        );
        p.child_at(&key, hash, PackedArg::Owned(packed), f)
    }

    /// rr-par2: [`Self::child_packed`] for a borrowed key and packed key (the
    /// packed key is cloned only when the key is admitted).
    pub(crate) fn child_packed_ref<E>(
        &mut self,
        key: &K,
        packed: &K::Packed,
        hash: u64,
        f: impl FnOnce(Child<'_, K, P, W>) -> Result<ChildAction<P, W>, E>,
    ) -> Result<Option<W>, E> {
        let Self::Packed(p) = self else {
            return self.child(key.clone(), f);
        };
        debug_assert_eq!(
            hash,
            p.hash(packed),
            "a prepacked key carries its frontier hash"
        );
        p.child_at(key, hash, PackedArg::Borrowed(packed), f)
    }

    /// One child contribution: `f` sees the pending entry of `key`, or its
    /// absence, and decides. Returns the removed weight on `Remove`.
    pub(crate) fn child<E>(
        &mut self,
        key: K,
        f: impl FnOnce(Child<'_, K, P, W>) -> Result<ChildAction<P, W>, E>,
    ) -> Result<Option<W>, E> {
        match self {
            Self::Classic(c) => match c.pending.entry(key) {
                Entry::Occupied(mut entry) => {
                    let pending = entry.get_mut();
                    let action = f(Child::Occupied {
                        phase: pending.position.phase,
                        priority: &pending.position.priority,
                        weight: &mut pending.weight,
                    })?;
                    Ok(match action {
                        ChildAction::Remove => Some(entry.remove().weight),
                        _ => None,
                    })
                }
                Entry::Vacant(entry) => {
                    if let ChildAction::Insert(pending) = f(Child::Vacant(entry.key()))? {
                        c.order.insert(Scheduled {
                            phase: Reverse(pending.position.phase),
                            priority: pending.position.priority.clone(),
                            key: entry.key().clone(),
                        });
                        entry.insert(pending);
                    }
                    Ok(None)
                }
            },
            Self::Packed(p) => {
                let packed = key.pack();
                let hash = p.hash(&packed);
                p.child_at(&key, hash, PackedArg::Owned(packed), f)
            }
        }
    }
}

impl<K: FrontierKey, P: Clone + Ord + Hash, W> Packed<K, P, W> {
    fn child_at<E>(
        &mut self,
        key: &K,
        hash: u64,
        packed: PackedArg<'_, K::Packed>,
        f: impl FnOnce(Child<'_, K, P, W>) -> Result<ChildAction<P, W>, E>,
    ) -> Result<Option<W>, E> {
        let p = self;
        let found = p.find(hash, packed.get());
        let current = p.current;
        if let Some(id) = found
            && let Some(slot) = p.slots[id as usize].as_mut()
            && let Some(weight) = slot.weight.as_mut()
        {
            let later = slot.phase as usize > current;
            let action = f(Child::Occupied {
                phase: slot.phase as usize,
                priority: p.priorities.get(slot.priority),
                weight,
            })?;
            return Ok(match action {
                ChildAction::Remove => {
                    if later {
                        p.later_cancels += 1;
                    }
                    let slot = p.slots[id as usize].as_mut().expect("live slot");
                    p.live -= 1;
                    slot.weight.take()
                }
                _ => {
                    if later {
                        p.later_merges += 1;
                    }
                    None
                }
            });
        }
        if let ChildAction::Insert(pending) = f(Child::Vacant(key))? {
            p.insert(hash, packed.into_owned(), pending);
        }
        Ok(None)
    }
}

/// A packed key passed by value or borrowed (cloned only when stored).
enum PackedArg<'a, C: Clone> {
    Owned(C),
    Borrowed(&'a C),
}

impl<C: Clone> PackedArg<'_, C> {
    fn get(&self) -> &C {
        match self {
            Self::Owned(packed) => packed,
            Self::Borrowed(packed) => packed,
        }
    }
    fn into_owned(self) -> C {
        match self {
            Self::Owned(packed) => packed,
            Self::Borrowed(packed) => packed.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::OrderedPosition;
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn powers(rng: &mut StdRng) -> Vec<i64> {
        let len = rng.random_range(1..=41);
        (0..len)
            .map(|_| match rng.random_range(0..20) {
                0 => rng.random_range(-1000..1000),
                1 => i64::from(i8::MIN) - rng.random_range(0..3),
                2 => i64::from(i8::MAX) + rng.random_range(0..3),
                _ => rng.random_range(-3..4),
            })
            .collect()
    }

    #[test]
    fn compact_powers_round_trip_and_order_as_vectors() {
        let mut rng = StdRng::seed_from_u64(7);
        let keys: Vec<Vec<i64>> = (0..400).map(|_| powers(&mut rng)).collect();
        for a in &keys {
            let pa = CompactPowers::new(a, false);
            assert_eq!(pa.to_vec(), *a);
            assert_eq!(
                matches!(pa, CompactPowers::Inline { .. }),
                a.len() <= INLINE_POWERS && a.iter().all(|&p| i8::try_from(p).is_ok())
            );
            for b in &keys {
                let pb = CompactPowers::new(b, false);
                assert_eq!(pa.cmp_powers(&pb), a.cmp(b));
                assert_eq!(pa == pb, a == b);
            }
        }
        assert_eq!(std::mem::size_of::<CompactPowers>(), 40);
    }

    /// The packed frontiers pop what the classic one pops, in its order,
    /// through inserts, merges, cancellations, returns and removals.
    #[test]
    #[allow(clippy::needless_borrows_for_generic_args)] // one closure feeds both frontiers
    fn packed_frontiers_pop_the_classic_sequence() {
        for (seed, mode) in [
            (1, FrontierMode::Compact),
            (2, FrontierMode::Phased),
            (3, FrontierMode::Phased),
        ] {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut classic: Frontier<IntegralKey, i64, i64> = Frontier::new(FrontierMode::Classic);
            let mut packed: Frontier<IntegralKey, i64, i64> = Frontier::new(mode);
            let phases = 6;
            let position = |key: &IntegralKey| -> (usize, i64) {
                let p = key.powers();
                (
                    (p[0].unsigned_abs() as usize) % phases,
                    p.iter().sum::<i64>() % 5,
                )
            };
            let random_key = |rng: &mut StdRng| {
                let len = rng.random_range(1..=4);
                let v: Vec<i64> = (0..len).map(|_| rng.random_range(-3..4)).collect();
                IntegralKey::try_new(v).unwrap()
            };
            for _ in 0..200 {
                let key = random_key(&mut rng);
                let (phase, priority) = position(&key);
                let pending = || Pending {
                    position: OrderedPosition { phase, priority },
                    weight: 1,
                };
                if classic.remove(&key).is_none() {
                    classic.insert(key.clone(), pending());
                    packed.insert(key, pending());
                } else {
                    packed.remove(&key).unwrap();
                    classic.insert(key.clone(), pending());
                    packed.insert(key, pending());
                }
            }
            for phase in 0..phases {
                let order_c: Vec<_> = classic.iter().map(|(s, w)| (s.into_owned(), *w)).collect();
                let order_p: Vec<_> = packed.iter().map(|(s, w)| (s.into_owned(), *w)).collect();
                assert_eq!(order_c, order_p);
                // rr-par2: speculation candidates below every threshold of
                // this phase equal the iterator's.
                for (threshold, _) in order_c.iter().filter(|(s, _)| s.phase.0 == phase) {
                    for n in [1, 3, 1000] {
                        let expected: Vec<_> = classic
                            .iter()
                            .take_while(|(s, _)| s.phase.0 == phase)
                            .skip_while(|(s, _)| **s >= *threshold)
                            .take(n)
                            .map(|(s, _)| {
                                (
                                    s.key.clone(),
                                    OrderedPosition {
                                        phase,
                                        priority: s.priority,
                                    },
                                )
                            })
                            .collect();
                        assert_eq!(packed.phase_candidates(phase, threshold, n), expected);
                        assert_eq!(classic.phase_candidates(phase, threshold, n), expected);
                    }
                }
                loop {
                    let (a, b) = (classic.pop_in_phase(phase), packed.pop_in_phase(phase));
                    assert_eq!(a, b);
                    let Some((node, _)) = a else { break };
                    for _ in 0..rng.random_range(0..6) {
                        let key = random_key(&mut rng);
                        let (p, priority) = position(&key);
                        if p < phase || (p == phase && priority >= node.priority) {
                            continue;
                        }
                        let w = rng.random_range(-2..3);
                        let mut decide = |found: Child<'_, IntegralKey, i64, i64>| {
                            Ok::<_, ()>(match found {
                                Child::Occupied { weight, .. } => {
                                    *weight += w;
                                    if *weight == 0 {
                                        ChildAction::Remove
                                    } else {
                                        ChildAction::Keep
                                    }
                                }
                                Child::Vacant(_) if w == 0 => ChildAction::Keep,
                                Child::Vacant(_) => ChildAction::Insert(Pending {
                                    position: OrderedPosition { phase: p, priority },
                                    weight: w,
                                }),
                            })
                        };
                        let rc = classic.child(key.clone(), &mut decide).unwrap();
                        let rp = packed.child(key, &mut decide).unwrap();
                        assert_eq!(rc, rp);
                        assert_eq!(classic.len(), packed.len());
                    }
                }
                assert_eq!(
                    classic.earlier_than(phase + 1),
                    packed.earlier_than(phase + 1)
                );
            }
            assert!(classic.is_empty() && packed.is_empty());
        }
    }
}
