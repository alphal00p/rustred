//! Necessary aggregate filters; native summary inclusion remains authoritative.
//! Groups contain lookup candidates only, never own queued obligations.
//!
//! Every scan visits the logical candidates of the historical layout in the
//! historical order (groups in vector order, blocks in ID order, IDs in
//! order, the minimum-ID early exit) and reports each of them to a visitor:
//! a run the struct-of-arrays prefilter rejected (`blocks`), or one candidate
//! for the exact predicate. The set, order and count of logical candidates,
//! and therefore every persisted counter, are those of the per-ID callback
//! scan this kernel replaced.

use rustred::solver::DomainPowerSummary;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

mod blocks;
use blocks::{AxisEnvelope, BLOCK_SIZE, BlockBox, Meta, range_mask};
pub(in super::super) use blocks::{Coordinates, Entry, LaneSource, Lanes, Probe};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in super::super) enum Upper {
    Finite(u128),
    Infinity,
}

impl Upper {
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::Infinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a >= b,
            (Self::Finite(_), Self::Infinity) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in super::super) enum Lower {
    NegativeInfinity,
    Finite(i128),
}

impl Lower {
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::NegativeInfinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a <= b,
            (Self::Finite(_), Self::NegativeInfinity) => false,
        }
    }
}

/// Tight native extrema, not optional raw input labels or finite sentinels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in super::super) enum Signature {
    Empty,
    Nonempty {
        positive: Upper,
        numerator: Upper,
        difference: Lower,
    },
}

impl Signature {
    pub(in super::super) fn of<const N: usize>(summary: &DomainPowerSummary<N>) -> Self {
        summary
            .extrema()
            .map_or(Self::Empty, |extrema| Self::Nonempty {
                positive: extrema
                    .positive_power()
                    .1
                    .map_or(Upper::Infinity, Upper::Finite),
                numerator: extrema
                    .numerator_rank()
                    .1
                    .map_or(Upper::Infinity, Upper::Finite),
                difference: extrema
                    .power_difference()
                    .0
                    .map_or(Lower::NegativeInfinity, Lower::Finite),
            })
    }

    /// A necessary condition only: Q subset C implies this test for (C,Q).
    pub(in super::super) fn may_contain(self, other: Self) -> bool {
        match (self, other) {
            (_, Self::Empty) => true,
            (Self::Empty, Self::Nonempty { .. }) => false,
            (
                Self::Nonempty {
                    positive: a,
                    numerator: r,
                    difference: d,
                },
                Self::Nonempty {
                    positive: b,
                    numerator: s,
                    difference: e,
                },
            ) => a.contains(b) && r.contains(s) && d.contains(e),
        }
    }
}

/// Receives a scan's logical candidates in order.
pub(in super::super) trait Visit {
    /// Consecutive candidates the prefilter rejected; bit j of `word` is set
    /// when `run[j]` failed the bit word (the rest failed only the lanes).
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str>;
    /// One candidate that passed the prefilter: its exact predicate.
    fn test(&mut self, id: usize) -> Result<bool, &'static str>;
}

/// The infallible visitor of reverse retirement.
pub(in super::super) trait Retire {
    fn rejected(&mut self, run: &[u32], word: u32);
    fn test(&mut self, id: usize) -> bool;
    /// `count` examined candidates that a helper-prepared set decided (they
    /// reach neither `rejected` nor `test`); telemetry only.
    fn decided(&mut self, _count: usize) {}
}

/// Every candidate is tested (an unfiltered probe never rejects).
#[cfg(test)]
pub(in super::super) struct Each<F>(pub F);

#[cfg(test)]
impl<F: FnMut(usize) -> Result<bool, &'static str>> Visit for Each<F> {
    fn rejected(&mut self, _: &[u32], _: u32) -> Result<(), &'static str> {
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        (self.0)(id)
    }
}

#[cfg(test)]
impl<F: FnMut(usize) -> bool> Retire for Each<F> {
    fn rejected(&mut self, _: &[u32], _: u32) {}
    fn test(&mut self, id: usize) -> bool {
        (self.0)(id)
    }
}

/// A scan's receiver of rejected runs and exact tests (`Visit` or `Retire`).
trait Sink {
    type Error;
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), Self::Error>;
    fn test(&mut self, id: usize) -> Result<bool, Self::Error>;
}

struct Fallible<'a, V>(&'a mut V);

impl<V: Visit> Sink for Fallible<'_, V> {
    type Error = &'static str;
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        self.0.rejected(run, word)
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        self.0.test(id)
    }
}

struct Infallible<'a, V>(&'a mut V);

impl<V: Retire> Sink for Infallible<'_, V> {
    type Error = std::convert::Infallible;
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), Self::Error> {
        self.0.rejected(run, word);
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, Self::Error> {
        Ok(self.0.test(id))
    }
}

/// Walk the slots of `pass` (ascending) within `start..end`, reporting the
/// rejected runs between them to `sink`. Returns the first slot whose test
/// succeeded when `stop_at_hit`, and the mask of successful slots.
#[inline]
fn visit_slots<S: Sink>(
    ids: &[u32; BLOCK_SIZE],
    words: u32,
    pass: u32,
    start: usize,
    end: usize,
    stop_at_hit: bool,
    sink: &mut S,
) -> Result<(Option<usize>, u32), S::Error> {
    let mut pending = pass & range_mask(start, end);
    let mut at = start;
    let mut hits = 0;
    while pending != 0 {
        let slot = pending.trailing_zeros() as usize;
        pending &= pending - 1;
        if slot > at {
            sink.rejected(&ids[at..slot], (!words & range_mask(at, slot)) >> at)?;
        }
        at = slot + 1;
        if sink.test(ids[slot] as usize)? {
            hits |= 1 << slot;
            if stop_at_hit {
                return Ok((Some(slot), hits));
            }
        }
    }
    if end > at {
        sink.rejected(&ids[at..end], (!words & range_mask(at, end)) >> at)?;
    }
    Ok((None, hits))
}

/// The CP5 image of one persisted block.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename = "Block")]
struct StoredBlock {
    ids: [usize; BLOCK_SIZE],
    len: usize,
    envelope: Vec<AxisEnvelope>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename = "Group")]
struct StoredGroup {
    signature: Signature,
    blocks: Vec<StoredBlock>,
    live: usize,
}

/// The CP5 image of an index, decoded before it is validated and rebuilt.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename = "AggregateIndex")]
pub(super) struct StoredIndex {
    groups: Vec<StoredGroup>,
    live: usize,
}

impl StoredIndex {
    pub(super) fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.live == 0
    }

    /// Structural validation (the historical `restore_positions` checks).
    pub(super) fn validate(&self, domain_count: usize) -> Result<(), String> {
        let mut signatures = std::collections::HashSet::new();
        let mut live = 0usize;
        for group in &self.groups {
            if !signatures.insert(group.signature) {
                return Err("duplicate checkpoint index signature".into());
            }
            let mut count = 0usize;
            let mut previous = None;
            for block in &group.blocks {
                if block.len > BLOCK_SIZE
                    || block.ids[..block.len]
                        .iter()
                        .any(|&id| id >= domain_count || id >= u32::MAX as usize)
                {
                    return Err("invalid checkpoint coordinate block".into());
                }
                // Dead slots hold 0 or a former live ID (retain compacts in
                // place), both below the domain count. The restored u32 slots
                // must re-encode them byte for byte, so anything else is
                // refused rather than silently rewritten.
                if block.ids[block.len..]
                    .iter()
                    .any(|&id| id >= domain_count || id >= u32::MAX as usize)
                {
                    return Err("invalid checkpoint stale block slot".into());
                }
                for &id in &block.ids[..block.len] {
                    if previous.is_some_and(|old| old >= id) {
                        return Err("unordered checkpoint index IDs".into());
                    }
                    previous = Some(id);
                    count += 1;
                }
            }
            if count != group.live {
                return Err("checkpoint index group count mismatch".into());
            }
            live = live
                .checked_add(count)
                .ok_or("checkpoint index count overflow")?;
        }
        if live != self.live {
            return Err("checkpoint index live count mismatch".into());
        }
        Ok(())
    }

    /// Every indexed ID, in group/block order.
    pub(super) fn for_each_id(&self, mut f: impl FnMut(usize)) {
        for group in &self.groups {
            for block in &group.blocks {
                block.ids[..block.len].iter().for_each(|&id| f(id));
            }
        }
    }
}

struct Group<const N: usize> {
    signature: Signature,
    /// Sequential block rows, parallel to `blocks`; increasing immutable IDs
    /// within and across blocks, no duplicates.
    meta: Vec<Meta<N>>,
    blocks: Vec<BlockBox<N>>,
    /// Preserve the aggregate-only index's O(1) maintenance preflight per group.
    live: usize,
}

/// Prepared insertion owns new storage until all queue preflights succeed.
pub(in super::super) struct Insertion<const N: usize> {
    signature: Signature,
    new_group: Option<Group<N>>,
    /// Prepared before responsibility mutation, or None when the existing tail
    /// has room and must be pinned through reverse retirement.
    new_block: Option<(Meta<N>, BlockBox<N>)>,
    /// Exact envelope storage for a narrow tail that the new coordinates
    /// widen beyond the narrow codes.
    spare_envelope: Option<Box<[AxisEnvelope]>>,
}

impl<const N: usize> Insertion<N> {
    #[cfg(test)]
    pub(super) fn has_new_block(&self) -> bool {
        self.new_block.is_some()
    }
}

pub(in super::super) struct AggregateIndex<const N: usize> {
    groups: Vec<Group<N>>,
    positions: HashMap<Signature, usize>,
    live: usize,
    /// Running totals behind `storage`, so a heartbeat reads them in O(1).
    totals: Totals,
    #[cfg(test)]
    work: WorkCounters,
}

impl<const N: usize> Default for AggregateIndex<N> {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            positions: HashMap::new(),
            live: 0,
            totals: Totals::default(),
            #[cfg(test)]
            work: WorkCounters::default(),
        }
    }
}

/// Storage totals kept current at every mutation that changes them (a
/// reserved capacity, the block count, exact envelope storage or a lossy live
/// slot), so that `storage` never walks the blocks. `recount` is the full walk
/// they must always equal (checked by the tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Totals {
    /// Boxed blocks (the sum of the groups' block counts).
    blocks: usize,
    /// Reserved bytes of the group vector, of every group's row and block
    /// pointer vectors, and of the exact (wide) envelopes.
    rows: usize,
    /// Live slots whose lanes saturate.
    lossy: usize,
}

/// Bytes a vector reserved since its capacity was `before` (capacities in
/// this index only grow while its vectors live).
fn grown<T>(vector: &Vec<T>, before: usize) -> usize {
    (vector.capacity() - before) * std::mem::size_of::<T>()
}

impl<const N: usize> Group<N> {
    /// Reserved bytes of the row and block pointer vectors.
    fn vector_bytes(&self) -> usize {
        self.meta.capacity() * std::mem::size_of::<Meta<N>>()
            + self.blocks.capacity() * std::mem::size_of::<BlockBox<N>>()
    }
}

#[cfg(test)]
struct WorkCounters {
    enabled: bool,
    groups_visited: AtomicUsize,
    groups_rejected: AtomicUsize,
    blocks_visited: AtomicUsize,
    blocks_rejected: AtomicUsize,
}

#[cfg(test)]
impl Default for WorkCounters {
    fn default() -> Self {
        Self {
            enabled: true,
            groups_visited: AtomicUsize::new(0),
            groups_rejected: AtomicUsize::new(0),
            blocks_visited: AtomicUsize::new(0),
            blocks_rejected: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
impl<const N: usize> super::Queue<N> {
    /// Disable only test instrumentation on existing and future indexes.
    /// The preference is not a persistent queue policy or production field.
    pub(in super::super) fn disable_index_work_counters(&mut self) {
        self.index_work_counters_enabled = false;
        for bucket in self.by_owner.values_mut() {
            bucket.indexed.work.enabled = false;
        }
    }

    /// A fresh restored fixture has zero counters. Do not reset or hide work
    /// already observed, or accept a new default-enabled bucket accidentally.
    pub(in super::super) fn index_work_counters_disabled_and_zero(&self) -> bool {
        !self.index_work_counters_enabled
            && self.by_owner.values().all(|bucket| {
                let index = &bucket.indexed;
                let work = index.work();
                !index.work.enabled
                    && work.groups_visited == 0
                    && work.groups_rejected == 0
                    && work.blocks_visited == 0
                    && work.blocks_rejected == 0
            })
    }
}

/// Test-replay instrumentation only; no additional production per-group work.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FilterWork {
    pub groups_visited: usize,
    pub groups_rejected: usize,
    pub blocks_visited: usize,
    pub blocks_rejected: usize,
}

/// Actual allocated block/envelope capacities, excluding group/hash/allocator
/// overhead and the separately retained domains and native summaries.
#[cfg(test)]
#[derive(Debug)]
struct BlockStorage {
    live_ids: usize,
    blocks: usize,
    id_slots: usize,
    block_capacity_bytes: usize,
    envelope_capacity_bytes: usize,
}

/// Reserved bytes of the index storage: group vectors, sequential block rows
/// and boxed blocks (allocator and hash-map overhead excluded).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct IndexBytes {
    pub blocks: usize,
    pub rows: usize,
    pub live: usize,
    pub lossy: usize,
}

impl<const N: usize> AggregateIndex<N> {
    #[cfg(test)]
    pub(super) fn set_work_counters_enabled(&mut self, enabled: bool) {
        self.work.enabled = enabled;
    }

    /// Rebuild a validated CP5 image. `entry` supplies the word and lanes of
    /// each live ID; group order, block partition, envelopes and stale slots
    /// are kept exactly.
    pub(super) fn restore(
        stored: StoredIndex,
        domain_count: usize,
        mut entry: impl FnMut(usize) -> Result<(u64, Option<Lanes<N>>), String>,
    ) -> Result<Self, String> {
        stored.validate(domain_count)?;
        let mut index = Self::default();
        index
            .groups
            .try_reserve_exact(stored.groups.len())
            .map_err(|_| "checkpoint index allocation")?;
        for (position, group) in stored.groups.into_iter().enumerate() {
            let mut meta = Vec::new();
            let mut blocks = Vec::new();
            meta.try_reserve_exact(group.blocks.len())
                .map_err(|_| "checkpoint index allocation")?;
            blocks
                .try_reserve_exact(group.blocks.len())
                .map_err(|_| "checkpoint index allocation")?;
            for block in &group.blocks {
                let (row, storage) = Meta::restore(
                    &block.ids,
                    block.len,
                    &block.envelope,
                    domain_count,
                    &mut entry,
                )?;
                meta.push(row);
                blocks.push(storage);
            }
            index.positions.insert(group.signature, position);
            index.groups.push(Group {
                signature: group.signature,
                meta,
                blocks,
                live: group.live,
            });
        }
        index.live = stored.live;
        // One walk at restore; every later change updates the totals.
        index.totals = index.recount();
        Ok(index)
    }

    /// Admission IDs are monotone within each group; skip the immutable prefix
    /// already disproved by a read-only snapshot lookup.
    pub(in super::super) fn find_from(
        &self,
        signature: Signature,
        probe: &Probe<'_, N>,
        first_id: usize,
        visit: &mut impl Visit,
    ) -> Result<Option<usize>, &'static str> {
        self.find_controlled(signature, probe, first_id, || Ok(()), visit)
    }

    /// Cancellation checkpoints are uncharged and also visit rejected blocks;
    /// speculative cancellation must not depend on reaching a native callback.
    pub(in super::super) fn find_controlled(
        &self,
        signature: Signature,
        probe: &Probe<'_, N>,
        first_id: usize,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
        visit: &mut impl Visit,
    ) -> Result<Option<usize>, &'static str> {
        let mut best: Option<usize> = None;
        for group in &self.groups {
            checkpoint()?;
            let eligible = group.signature.may_contain(signature);
            #[cfg(test)]
            self.record_group(eligible);
            if !eligible {
                continue;
            }
            // IDs increase across blocks: the blocks wholly below `first_id`
            // form a prefix (an empty block is never counted in it). The
            // bounds are checked first: an unbounded scan skips nothing, and a
            // revalidation usually finds every block below its watermark.
            let skip = if first_id == 0 {
                0
            } else if group
                .meta
                .last()
                .is_some_and(|meta| (meta.last as usize) < first_id)
            {
                group.meta.len()
            } else {
                group
                    .meta
                    .partition_point(|meta| (meta.last as usize) < first_id)
            };
            for (meta, block) in group.meta[skip..].iter().zip(&group.blocks[skip..]) {
                checkpoint()?;
                if meta.len == 0 || (meta.last as usize) < first_id {
                    continue;
                }
                // Group order may change during retirement. Minimum admission
                // ID, not hash/vector traversal order, determines the result.
                if best.is_some_and(|best| meta.first as usize >= best) {
                    break;
                }
                let eligible = meta.may_contain(probe);
                #[cfg(test)]
                self.record_block(eligible);
                if !eligible {
                    continue;
                }
                let block = &block[0];
                let len = meta.len as usize;
                let ids = &block.ids;
                let start = if first_id <= meta.first as usize {
                    0
                } else {
                    ids[..len].partition_point(|&id| (id as usize) < first_id)
                };
                let end = best.map_or(len, |best| {
                    ids[..len].partition_point(|&id| (id as usize) < best)
                });
                if start >= end {
                    continue;
                }
                let (words, pass) = block.forward(probe, len);
                let (hit, _) =
                    visit_slots(ids, words, pass, start, end, true, &mut Fallible(visit))?;
                if let Some(slot) = hit {
                    best = Some(ids[slot] as usize);
                }
            }
        }
        Ok(best)
    }

    /// Read-only live-ID streaming for an immutable snapshot bootstrap. The
    /// receiver can stop without allocating a campaign-sized temporary list.
    pub(in super::super) fn visit_live(
        &self,
        mut visit: impl FnMut(u32) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        for group in &self.groups {
            for (meta, block) in group.meta.iter().zip(&group.blocks) {
                for &id in &block[0].ids[..meta.len as usize] {
                    visit(id)?;
                }
            }
        }
        Ok(())
    }

    /// Every indexed (live) candidate ID, in group/block order.
    #[cfg(test)]
    pub(super) fn for_each_id(&self, mut f: impl FnMut(usize)) {
        for group in &self.groups {
            for (meta, block) in group.meta.iter().zip(&group.blocks) {
                block[0].ids[..meta.len as usize]
                    .iter()
                    .for_each(|&id| f(id as usize));
            }
        }
    }

    pub(in super::super) fn is_live(&self, signature: Signature, id: usize) -> bool {
        let Ok(id) = u32::try_from(id) else {
            return false;
        };
        self.positions.get(&signature).is_some_and(|&position| {
            let group = &self.groups[position];
            let block = group.meta.partition_point(|meta| meta.last < id);
            group.meta.get(block).is_some_and(|meta| {
                group.blocks[block][0].ids[..meta.len as usize]
                    .binary_search(&id)
                    .is_ok()
            })
        })
    }

    pub(in super::super) fn prepare(
        &mut self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
    ) -> Result<Insertion<N>, &'static str> {
        self.prepare_with(signature, coordinates, || Ok(()))
    }

    // The inlined no-op checkpoint lets tests fault-inject each reservation
    // without changing the production storage policy or allocating huge memory.
    pub(in super::super) fn prepare_with(
        &mut self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Insertion<N>, &'static str> {
        self.live
            .checked_add(1)
            .ok_or("candidate index count overflow")?;
        let mut spare_envelope = None;
        let (new_group, new_block) = if let Some(&position) = self.positions.get(&signature) {
            let group = &mut self.groups[position];
            if let Some(tail) = group.meta.last().filter(|meta| meta.has_room()) {
                if tail.needs_wide_storage(coordinates) {
                    checkpoint()?;
                    let coordinates = coordinates.expect("coordinates that need storage");
                    spare_envelope = Some(Meta::<N>::wide_storage(coordinates)?);
                }
                (None, None)
            } else {
                checkpoint()?;
                // A reservation that succeeds stays reserved even if a later
                // preflight fails, so account it immediately.
                let (rows, pointers) = (group.meta.capacity(), group.blocks.capacity());
                let reserved = group
                    .meta
                    .try_reserve(1)
                    .and_then(|()| group.blocks.try_reserve(1));
                self.totals.rows += grown(&group.meta, rows) + grown(&group.blocks, pointers);
                reserved.map_err(|_| "coordinate block allocation")?;
                (None, Some(Meta::prepare(coordinates, &mut checkpoint)?))
            }
        } else {
            checkpoint()?;
            let before = self.groups.capacity();
            let reserved = self.groups.try_reserve(1);
            self.totals.rows += grown(&self.groups, before);
            reserved.map_err(|_| "aggregate group allocation")?;
            checkpoint()?;
            self.positions
                .try_reserve(1)
                .map_err(|_| "aggregate group index allocation")?;
            let mut meta = Vec::new();
            let mut blocks = Vec::new();
            checkpoint()?;
            meta.try_reserve(1)
                .map_err(|_| "coordinate block allocation")?;
            blocks
                .try_reserve(1)
                .map_err(|_| "coordinate block allocation")?;
            let block = Meta::prepare(coordinates, &mut checkpoint)?;
            (
                Some(Group {
                    signature,
                    meta,
                    blocks,
                    live: 0,
                }),
                Some(block),
            )
        };
        Ok(Insertion {
            signature,
            new_group,
            new_block,
            spare_envelope,
        })
    }

    /// Conservative preflight/accounting bound retained from the aggregate-only
    /// index. Coordinate blocks can avoid actual callbacks but do not reduce
    /// these historical reverse-maintenance charges or overflow safeguards.
    pub(super) fn maintenance_len(&self, signature: Signature) -> Result<usize, &'static str> {
        self.groups
            .iter()
            .filter(|group| {
                let eligible = signature.may_contain(group.signature);
                #[cfg(test)]
                self.record_group(eligible);
                eligible
            })
            .try_fold(0_usize, |count, group| {
                count
                    .checked_add(group.live)
                    .ok_or("candidate index count overflow")
            })
    }

    /// Read-only twin of `retire` for speculative preparation: the ascending
    /// IDs that the visitor accepts among the candidates the reverse pass would
    /// examine (same group and block eligibility). Nothing is mutated and no
    /// work counter of the queue is charged here. Cancellation checkpoints run
    /// at every group and block boundary, as in `find_controlled`. More than
    /// `limit` accepted IDs is an error so a pathological snapshot never holds
    /// an unbounded helper allocation.
    pub(in super::super) fn collect_contained(
        &self,
        signature: Signature,
        probe: &Probe<'_, N>,
        limit: usize,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
        visit: &mut impl Visit,
    ) -> Result<Vec<usize>, &'static str> {
        let mut contained = Vec::new();
        for group in &self.groups {
            checkpoint()?;
            let eligible = signature.may_contain(group.signature);
            #[cfg(test)]
            self.record_group(eligible);
            if !eligible {
                continue;
            }
            for (meta, block) in group.meta.iter().zip(&group.blocks) {
                checkpoint()?;
                let eligible = meta.may_be_contained(probe);
                #[cfg(test)]
                self.record_block(eligible);
                if !eligible || meta.len == 0 {
                    continue;
                }
                let block = &block[0];
                let len = meta.len as usize;
                let (words, pass) = block.reverse(probe, len);
                let (_, hits) =
                    visit_slots(&block.ids, words, pass, 0, len, false, &mut Fallible(visit))?;
                let mut hits = hits;
                while hits != 0 {
                    let slot = hits.trailing_zeros() as usize;
                    hits &= hits - 1;
                    if contained.len() >= limit {
                        return Err("prepared retirement set limit");
                    }
                    contained
                        .try_reserve(1)
                        .map_err(|_| "prepared retirement set allocation")?;
                    contained.push(block.ids[slot] as usize);
                }
            }
        }
        contained.sort_unstable();
        Ok(contained)
    }

    /// `retire` with its per-ID decision split between a snapshot-prepared set
    /// (IDs below `first_new`, sorted ascending) and `contains_new` for IDs
    /// admitted at or after the snapshot watermark. It runs the very same
    /// traversal, retain, tail pinning and group removal as `retire`, so the
    /// resulting layout is identical whenever the prepared set agrees with the
    /// exact predicate on every examined old ID (see `prepared` for the
    /// argument). `on_retire` observes each removal in traversal order.
    /// `visit_new` sees the prefilter rejections of IDs at or above
    /// `first_new` only (the old IDs are decided by the set).
    pub(super) fn retire_prepared(
        &mut self,
        insertion: &Insertion<N>,
        probe: &Probe<'_, N>,
        prepared: &[usize],
        first_new: usize,
        visit_new: &mut impl Retire,
        mut on_retire: impl FnMut(usize),
    ) -> usize {
        debug_assert!(prepared.windows(2).all(|pair| pair[0] < pair[1]));
        debug_assert!(prepared.last().is_none_or(|&last| last < first_new));
        struct Split<'a, V, F> {
            prepared: &'a [usize],
            first_new: usize,
            visit_new: &'a mut V,
            on_retire: F,
        }
        impl<V: Retire, F: FnMut(usize)> Retire for Split<'_, V, F> {
            fn rejected(&mut self, run: &[u32], word: u32) {
                // A run is ascending; runs wholly below the watermark (the
                // usual case) need no search.
                let old = if run.last().is_none_or(|&id| (id as usize) < self.first_new) {
                    run.len()
                } else {
                    run.partition_point(|&id| (id as usize) < self.first_new)
                };
                if old > 0 {
                    self.visit_new.decided(old);
                }
                if old < run.len() {
                    self.visit_new.rejected(&run[old..], word >> old);
                }
            }
            fn test(&mut self, id: usize) -> bool {
                let retire = if id < self.first_new {
                    self.visit_new.decided(1);
                    self.prepared.binary_search(&id).is_ok()
                } else {
                    self.visit_new.test(id)
                };
                if retire {
                    (self.on_retire)(id);
                }
                retire
            }
        }
        self.retire(
            insertion,
            probe,
            &mut Split {
                prepared,
                first_new,
                visit_new,
                on_retire: &mut on_retire,
            },
        )
    }

    /// Infallible after queue counter/storage preflight. Preserve the insertion
    /// signature's reserved group even if it becomes empty; remove other empty
    /// groups so historical signatures do not accumulate in the hot scan.
    pub(in super::super) fn retire(
        &mut self,
        insertion: &Insertion<N>,
        probe: &Probe<'_, N>,
        visit: &mut impl Retire,
    ) -> usize {
        let mut removed = 0;
        let mut position = 0;
        while position < self.groups.len() {
            let eligible = insertion
                .signature
                .may_contain(self.groups[position].signature);
            #[cfg(test)]
            self.record_group(eligible);
            let group = &mut self.groups[position];
            if eligible {
                // The first empty row after the scan: rows before it are all
                // kept in place, so compaction starts there (usually nowhere).
                let mut first_empty = None;
                for (row, (meta, block)) in group.meta.iter_mut().zip(&mut group.blocks).enumerate()
                {
                    let eligible = meta.may_be_contained(probe);
                    #[cfg(test)]
                    if self.work.enabled {
                        self.work.blocks_visited.fetch_add(1, Ordering::Relaxed);
                        self.work
                            .blocks_rejected
                            .fetch_add(usize::from(!eligible), Ordering::Relaxed);
                    }
                    if eligible && meta.len != 0 {
                        let block = &mut block[0];
                        let len = meta.len as usize;
                        let (words, pass) = block.reverse(probe, len);
                        let Ok((_, hits)) = visit_slots(
                            &block.ids,
                            words,
                            pass,
                            0,
                            len,
                            false,
                            &mut Infallible(visit),
                        );
                        if hits != 0 {
                            let lossy = block.lossy(len).count_ones();
                            let gone = meta.retain(block, !hits);
                            self.totals.lossy -=
                                (lossy - block.lossy(meta.len as usize).count_ones()) as usize;
                            removed += gone;
                            group.live -= gone;
                        }
                    }
                    if meta.len == 0 && first_empty.is_none() {
                        first_empty = Some(row);
                    }
                }
                let pin_tail =
                    group.signature == insertion.signature && insertion.new_block.is_none();
                let old_len = group.meta.len();
                // Stable compaction of the non-empty rows (and a pinned tail).
                // Rows below `first_empty` never move, and a row moves only
                // when an earlier one is dropped: a retirement that empties no
                // block copies nothing (runB: the unconditional 144-B row swap
                // took ~30% of coordinator samples). Same layout as swapping
                // every kept row into place.
                let mut kept = first_empty.unwrap_or(old_len);
                for read in kept..old_len {
                    if group.meta[read].len != 0 || (pin_tail && read + 1 == old_len) {
                        if kept != read {
                            group.meta.swap(kept, read);
                            group.blocks.swap(kept, read);
                        }
                        kept += 1;
                    }
                }
                // Dropped rows free their boxed blocks and exact envelopes;
                // the row and pointer vectors keep their capacity.
                self.totals.blocks -= old_len - kept;
                self.totals.rows -= group.meta[kept..]
                    .iter()
                    .map(Meta::envelope_capacity_bytes)
                    .sum::<usize>();
                group.meta.truncate(kept);
                group.blocks.truncate(kept);
            }
            if group.meta.is_empty() && group.signature != insertion.signature {
                let key = group.signature;
                self.totals.rows -= group.vector_bytes();
                self.groups.swap_remove(position);
                self.positions.remove(&key);
                if let Some(moved) = self.groups.get(position) {
                    *self
                        .positions
                        .get_mut(&moved.signature)
                        .expect("indexed group") = position;
                }
            } else {
                position += 1;
            }
        }
        self.live -= removed;
        removed
    }

    pub(in super::super) fn insert(&mut self, mut insertion: Insertion<N>, entry: Entry<'_, N>) {
        let spare = insertion.spare_envelope.take();
        let lossy = entry.lanes.as_ref().is_some_and(|lanes| lanes.lossy);
        if let Some(mut group) = insertion.new_group.take() {
            let (mut meta, mut block) = insertion
                .new_block
                .take()
                .expect("preallocated new group block");
            meta.push(&mut block[0], entry, spare);
            let envelope = meta.envelope_capacity_bytes();
            group.meta.push(meta);
            group.blocks.push(block);
            group.live = 1;
            self.positions
                .insert(insertion.signature, self.groups.len());
            let before = self.groups.capacity();
            self.totals.rows += group.vector_bytes() + envelope;
            self.groups.push(group);
            self.totals.rows += grown(&self.groups, before);
            self.totals.blocks += 1;
        } else {
            let position = self.positions[&insertion.signature];
            let group = &mut self.groups[position];
            // Exact envelope bytes of the target row already counted.
            let counted = if let Some((meta, block)) = insertion.new_block.take() {
                let (rows, pointers) = (group.meta.capacity(), group.blocks.capacity());
                group.meta.push(meta);
                group.blocks.push(block);
                self.totals.rows += grown(&group.meta, rows) + grown(&group.blocks, pointers);
                self.totals.blocks += 1;
                0
            } else {
                group
                    .meta
                    .last()
                    .expect("reserved tail block")
                    .envelope_capacity_bytes()
            };
            let meta = group.meta.last_mut().expect("reserved tail block");
            let block = group.blocks.last_mut().expect("reserved tail block");
            meta.push(&mut block[0], entry, spare);
            // A narrow envelope may have become exact (wide) storage.
            self.totals.rows += meta.envelope_capacity_bytes() - counted;
            group.live += 1; // bounded by checked global live + 1
        }
        self.totals.lossy += usize::from(lossy);
        self.live += 1; // checked by prepare before any retirement
    }

    /// Reserved storage of the index (see `IndexBytes`), from the running
    /// totals: O(1), cheap enough for every coordinator heartbeat.
    pub(in super::super) fn storage(&self) -> IndexBytes {
        IndexBytes {
            blocks: self.totals.blocks * std::mem::size_of::<blocks::Block<N>>(),
            rows: self.totals.rows,
            live: self.live,
            lossy: self.totals.lossy,
        }
    }

    /// The totals by a full walk of every group and block (O(blocks), one
    /// pointer dereference per boxed block): restore only, and the tests'
    /// reference for the running totals.
    fn recount(&self) -> Totals {
        let mut totals = Totals {
            rows: self.groups.capacity() * std::mem::size_of::<Group<N>>(),
            ..Totals::default()
        };
        for group in &self.groups {
            totals.rows += group.vector_bytes()
                + group
                    .meta
                    .iter()
                    .map(Meta::envelope_capacity_bytes)
                    .sum::<usize>();
            totals.blocks += group.blocks.len();
            for (meta, block) in group.meta.iter().zip(&group.blocks) {
                totals.lossy += block[0].lossy(meta.len as usize).count_ones() as usize;
            }
        }
        totals
    }

    /// `storage` recomputed by the full walk (tests and the gen-7 harness).
    #[cfg(test)]
    pub(super) fn storage_by_walk(&self) -> IndexBytes {
        let totals = self.recount();
        IndexBytes {
            blocks: totals.blocks * std::mem::size_of::<blocks::Block<N>>(),
            rows: totals.rows,
            live: self.live,
            lossy: totals.lossy,
        }
    }

    #[cfg(test)]
    pub(in super::super) fn ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        self.for_each_id(|id| ids.push(id));
        ids.sort_unstable();
        ids
    }

    #[cfg(test)]
    pub(super) fn groups(&self) -> usize {
        self.groups.len()
    }

    /// Exact physical layout: group order, signatures and block contents.
    #[cfg(test)]
    pub(super) fn layout(&self) -> Vec<(Signature, Vec<Vec<usize>>)> {
        self.groups
            .iter()
            .map(|group| {
                (
                    group.signature,
                    group
                        .meta
                        .iter()
                        .zip(&group.blocks)
                        .map(|(meta, block)| {
                            block[0].ids[..meta.len as usize]
                                .iter()
                                .map(|&id| id as usize)
                                .collect()
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[cfg(test)]
    fn block_storage(&self) -> BlockStorage {
        let blocks = self.groups.iter().map(|group| group.blocks.len()).sum();
        BlockStorage {
            live_ids: self.live,
            blocks,
            id_slots: blocks * BLOCK_SIZE,
            block_capacity_bytes: self
                .groups
                .iter()
                .map(|group| {
                    group.blocks.capacity() * std::mem::size_of::<BlockBox<N>>()
                        + group.blocks.len() * std::mem::size_of::<blocks::Block<N>>()
                })
                .sum(),
            envelope_capacity_bytes: self
                .groups
                .iter()
                .map(|group| {
                    group.meta.capacity() * std::mem::size_of::<Meta<N>>()
                        + group
                            .meta
                            .iter()
                            .map(Meta::envelope_capacity_bytes)
                            .sum::<usize>()
                })
                .sum(),
        }
    }

    #[cfg(test)]
    fn record_group(&self, eligible: bool) {
        if !self.work.enabled {
            return;
        }
        self.work.groups_visited.fetch_add(1, Ordering::Relaxed);
        self.work
            .groups_rejected
            .fetch_add(usize::from(!eligible), Ordering::Relaxed);
    }

    #[cfg(test)]
    fn record_block(&self, eligible: bool) {
        if !self.work.enabled {
            return;
        }
        self.work.blocks_visited.fetch_add(1, Ordering::Relaxed);
        self.work
            .blocks_rejected
            .fetch_add(usize::from(!eligible), Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(super) fn work(&self) -> FilterWork {
        FilterWork {
            groups_visited: self.work.groups_visited.load(Ordering::Relaxed),
            groups_rejected: self.work.groups_rejected.load(Ordering::Relaxed),
            blocks_visited: self.work.blocks_visited.load(Ordering::Relaxed),
            blocks_rejected: self.work.blocks_rejected.load(Ordering::Relaxed),
        }
    }
}

/// The pre-kernel per-candidate callback API over an unfiltered probe: the
/// index-level tests exercise traversal, charging order and layout with it.
#[cfg(test)]
impl<const N: usize> AggregateIndex<N> {
    pub(super) fn find_each(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        self.find_from_each(signature, coordinates, 0, contains)
    }

    pub(super) fn find_from_each(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        first_id: usize,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        self.find_controlled_each(signature, coordinates, first_id, || Ok(()), contains)
    }

    pub(super) fn find_controlled_each(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        first_id: usize,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        self.find_controlled(
            signature,
            &Probe::unfiltered(coordinates),
            first_id,
            checkpoint,
            &mut Each(contains),
        )
    }

    pub(super) fn collect_contained_each(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        limit: usize,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Vec<usize>, &'static str> {
        self.collect_contained(
            signature,
            &Probe::unfiltered(coordinates),
            limit,
            checkpoint,
            &mut Each(contains),
        )
    }

    pub(super) fn retire_each(
        &mut self,
        insertion: &Insertion<N>,
        coordinates: Option<Coordinates<'_>>,
        contains: impl FnMut(usize) -> bool,
    ) -> usize {
        self.retire(
            insertion,
            &Probe::unfiltered(coordinates),
            &mut Each(contains),
        )
    }

    pub(super) fn retire_prepared_each(
        &mut self,
        insertion: &Insertion<N>,
        coordinates: Option<Coordinates<'_>>,
        prepared: &[usize],
        first_new: usize,
        contains_new: impl FnMut(usize) -> bool,
        on_retire: impl FnMut(usize),
    ) -> usize {
        self.retire_prepared(
            insertion,
            &Probe::unfiltered(coordinates),
            prepared,
            first_new,
            &mut Each(contains_new),
            on_retire,
        )
    }

    /// Insert without lanes (an escaped candidate) and with word 0.
    pub(super) fn insert_plain(
        &mut self,
        insertion: Insertion<N>,
        id: usize,
        coordinates: Option<Coordinates<'_>>,
    ) {
        self.insert(
            insertion,
            Entry {
                id,
                coordinates,
                word: 0,
                lanes: None,
            },
        );
    }

    pub(super) fn block_ids(&self, group: usize, block: usize) -> Vec<usize> {
        let group = &self.groups[group];
        group.blocks[block][0].ids[..group.meta[block].len as usize]
            .iter()
            .map(|&id| id as usize)
            .collect()
    }

    /// Every block's live kernel data (ID, word, lanes) and OR/AND words, in
    /// layout order.
    #[allow(clippy::type_complexity)]
    pub(super) fn kernel_image(&self) -> Vec<(Vec<(u32, u64, Option<Lanes<N>>)>, (u64, u64))> {
        self.groups
            .iter()
            .flat_map(|group| group.meta.iter().zip(&group.blocks))
            .map(|(meta, block)| {
                let block = &block[0];
                (
                    (0..meta.len as usize)
                        .map(|slot| block.entry(slot))
                        .collect(),
                    block.block_words(),
                )
            })
            .collect()
    }

    /// The kernel on every live slot of every block, both directions:
    /// `f(id, (forward word, forward pass), (reverse word, reverse pass),
    /// lossy or escaped)`. Group and envelope filters are bypassed.
    pub(super) fn sweep(
        &self,
        probe: &Probe<'_, N>,
        mut f: impl FnMut(usize, (bool, bool), (bool, bool), bool),
    ) {
        for group in &self.groups {
            for (meta, block) in group.meta.iter().zip(&group.blocks) {
                let block = &block[0];
                let len = meta.len as usize;
                let (fw, fp) = block.forward(probe, len);
                let (rw, rp) = block.reverse(probe, len);
                let inexact = block.lossy(len) | block.escaped(len);
                for slot in 0..len {
                    let bit = 1 << slot;
                    f(
                        block.ids[slot] as usize,
                        (fw & bit != 0, fp & bit != 0),
                        (rw & bit != 0, rp & bit != 0),
                        inexact & bit != 0,
                    );
                }
            }
        }
    }

    pub(super) fn block_lens(&self, group: usize) -> Vec<usize> {
        self.groups[group]
            .meta
            .iter()
            .map(|meta| meta.len as usize)
            .collect()
    }
}

/// The CP5 image, written from the live layout without an intermediate copy.
impl<const N: usize> serde::Serialize for AggregateIndex<N> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeSeq, SerializeStruct};
        struct Blocks<'a, const N: usize>(&'a Group<N>);
        struct BlockImage<'a, const N: usize>(&'a Meta<N>, &'a blocks::Block<N>);
        struct Envelope<'a, const N: usize>(&'a Meta<N>);
        struct Groups<'a, const N: usize>(&'a [Group<N>]);
        struct GroupImage<'a, const N: usize>(&'a Group<N>);
        impl<const N: usize> serde::Serialize for Envelope<'_, N> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let axes = self.0.envelope_axes();
                let mut seq = s.serialize_seq(Some(axes.len()))?;
                for axis in axes {
                    seq.serialize_element(&axis)?;
                }
                seq.end()
            }
        }
        impl<const N: usize> serde::Serialize for BlockImage<'_, N> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let ids: [usize; BLOCK_SIZE] =
                    std::array::from_fn(|slot| self.1.ids[slot] as usize);
                let mut image = s.serialize_struct("Block", 3)?;
                image.serialize_field("ids", &ids)?;
                image.serialize_field("len", &(self.0.len as usize))?;
                image.serialize_field("envelope", &Envelope(self.0))?;
                image.end()
            }
        }
        impl<const N: usize> serde::Serialize for Blocks<'_, N> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut seq = s.serialize_seq(Some(self.0.meta.len()))?;
                for (meta, block) in self.0.meta.iter().zip(&self.0.blocks) {
                    seq.serialize_element(&BlockImage(meta, &block[0]))?;
                }
                seq.end()
            }
        }
        impl<const N: usize> serde::Serialize for GroupImage<'_, N> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut image = s.serialize_struct("Group", 3)?;
                image.serialize_field("signature", &self.0.signature)?;
                image.serialize_field("blocks", &Blocks(self.0))?;
                image.serialize_field("live", &self.0.live)?;
                image.end()
            }
        }
        impl<const N: usize> serde::Serialize for Groups<'_, N> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut seq = s.serialize_seq(Some(self.0.len()))?;
                for group in self.0 {
                    seq.serialize_element(&GroupImage(group))?;
                }
                seq.end()
            }
        }
        let mut image = s.serialize_struct("AggregateIndex", 2)?;
        image.serialize_field("groups", &Groups(&self.groups))?;
        image.serialize_field("live", &self.live)?;
        image.end()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod work_counter_tests;
