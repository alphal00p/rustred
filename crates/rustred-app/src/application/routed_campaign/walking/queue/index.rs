//! Necessary aggregate filters; native summary inclusion remains authoritative.
//! Groups contain lookup candidates only, never own queued obligations.

use rustred::solver::DomainPowerSummary;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

mod blocks;
use blocks::Block;
pub(super) use blocks::Coordinates;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(super) enum Upper {
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
pub(super) enum Lower {
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
pub(super) enum Signature {
    Empty,
    Nonempty {
        positive: Upper,
        numerator: Upper,
        difference: Lower,
    },
}

impl Signature {
    pub(super) fn of<const N: usize>(summary: &DomainPowerSummary<N>) -> Self {
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
    fn may_contain(self, other: Self) -> bool {
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

impl AggregateIndex {
    pub(super) fn restore_positions(&mut self, domain_count: usize) -> Result<(), String> {
        self.positions.clear();
        let mut live = 0usize;
        for (position, group) in self.groups.iter().enumerate() {
            if self.positions.insert(group.signature, position).is_some() {
                return Err("duplicate checkpoint index signature".into());
            }
            let mut count = 0usize;
            let mut previous = None;
            for block in &group.blocks {
                block.validate(domain_count)?;
                for &id in block.ids() {
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
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Group {
    signature: Signature,
    /// Increasing immutable IDs within and across blocks; no duplicates.
    blocks: Vec<Block>,
    /// Preserve the aggregate-only index's O(1) maintenance preflight per group.
    live: usize,
}

/// Prepared insertion owns new storage until all queue preflights succeed.
pub(super) struct Insertion {
    signature: Signature,
    new_group: Option<Group>,
    /// Prepared before responsibility mutation, or None when the existing tail
    /// has room and must be pinned through reverse retirement.
    new_block: Option<Block>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct AggregateIndex {
    groups: Vec<Group>,
    #[serde(skip)]
    positions: HashMap<Signature, usize>,
    live: usize,
    #[cfg(test)]
    #[serde(skip)]
    work: WorkCounters,
}

#[cfg(test)]
#[derive(Default)]
struct WorkCounters {
    groups_visited: AtomicUsize,
    groups_rejected: AtomicUsize,
    blocks_visited: AtomicUsize,
    blocks_rejected: AtomicUsize,
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

impl AggregateIndex {
    pub(super) fn find(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        self.find_from(signature, coordinates, 0, contains)
    }

    /// Admission IDs are monotone within each group; skip the immutable prefix
    /// already disproved by a read-only snapshot lookup.
    pub(super) fn find_from(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        first_id: usize,
        contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        self.find_controlled(signature, coordinates, first_id, || Ok(()), contains)
    }

    /// Cancellation checkpoints are uncharged and also visit rejected blocks;
    /// speculative cancellation must not depend on reaching a native callback.
    pub(super) fn find_controlled(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        first_id: usize,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
        mut contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        let mut best = None;
        for group in &self.groups {
            checkpoint()?;
            let eligible = group.signature.may_contain(signature);
            #[cfg(test)]
            self.record_group(eligible);
            if !eligible {
                continue;
            }
            for block in &group.blocks {
                checkpoint()?;
                let ids = block.ids();
                if ids.last().is_none_or(|&id| id < first_id) {
                    continue;
                }
                // Group order may change during retirement. Minimum admission
                // ID, not hash/vector traversal order, determines the result.
                if best.is_some_and(|best| ids[0] >= best) {
                    break;
                }
                let eligible = block.may_contain(coordinates);
                #[cfg(test)]
                self.record_block(eligible);
                if !eligible {
                    continue;
                }
                let start = ids.partition_point(|&id| id < first_id);
                for &id in &ids[start..] {
                    if best.is_some_and(|best| id >= best) {
                        break;
                    }
                    if contains(id)? {
                        best = Some(id);
                        break;
                    }
                }
            }
        }
        Ok(best)
    }

    pub(super) fn is_live(&self, signature: Signature, id: usize) -> bool {
        self.positions.get(&signature).is_some_and(|&position| {
            let blocks = &self.groups[position].blocks;
            let block =
                blocks.partition_point(|block| block.ids().last().is_some_and(|&last| last < id));
            blocks
                .get(block)
                .is_some_and(|block| block.ids().binary_search(&id).is_ok())
        })
    }

    pub(super) fn prepare(
        &mut self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
    ) -> Result<Insertion, &'static str> {
        self.prepare_with(signature, coordinates, || Ok(()))
    }

    // The inlined no-op checkpoint lets tests fault-inject each reservation
    // without changing the production storage policy or allocating huge memory.
    fn prepare_with(
        &mut self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Insertion, &'static str> {
        self.live
            .checked_add(1)
            .ok_or("candidate index count overflow")?;
        let (new_group, new_block) = if let Some(&position) = self.positions.get(&signature) {
            let group = &mut self.groups[position];
            if group.blocks.last().is_some_and(Block::has_room) {
                (None, None)
            } else {
                checkpoint()?;
                group
                    .blocks
                    .try_reserve(1)
                    .map_err(|_| "coordinate block allocation")?;
                (None, Some(Block::prepare(coordinates, &mut checkpoint)?))
            }
        } else {
            checkpoint()?;
            self.groups
                .try_reserve(1)
                .map_err(|_| "aggregate group allocation")?;
            checkpoint()?;
            self.positions
                .try_reserve(1)
                .map_err(|_| "aggregate group index allocation")?;
            let mut blocks = Vec::new();
            checkpoint()?;
            blocks
                .try_reserve(1)
                .map_err(|_| "coordinate block allocation")?;
            let block = Block::prepare(coordinates, &mut checkpoint)?;
            (
                Some(Group {
                    signature,
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

    /// Infallible after queue counter/storage preflight. Preserve the insertion
    /// signature's reserved group even if it becomes empty; remove other empty
    /// groups so historical signatures do not accumulate in the hot scan.
    pub(super) fn retire(
        &mut self,
        insertion: &Insertion,
        coordinates: Option<Coordinates<'_>>,
        mut contains: impl FnMut(usize) -> bool,
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
                for block in &mut group.blocks {
                    let eligible = block.may_be_contained(coordinates);
                    #[cfg(test)]
                    {
                        self.work.blocks_visited.fetch_add(1, Ordering::Relaxed);
                        self.work
                            .blocks_rejected
                            .fetch_add(usize::from(!eligible), Ordering::Relaxed);
                    }
                    if eligible {
                        let previous = block.ids().len();
                        block.retain(|id| !contains(id));
                        removed += previous - block.ids().len();
                        group.live -= previous - block.ids().len();
                    }
                }
                let pin_tail =
                    group.signature == insertion.signature && insertion.new_block.is_none();
                let old_len = group.blocks.len();
                let mut block_position = 0;
                group.blocks.retain(|block| {
                    block_position += 1;
                    !block.ids().is_empty() || (pin_tail && block_position == old_len)
                });
            }
            if group.blocks.is_empty() && group.signature != insertion.signature {
                let key = group.signature;
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

    pub(super) fn insert(
        &mut self,
        mut insertion: Insertion,
        id: usize,
        coordinates: Option<Coordinates<'_>>,
    ) {
        if let Some(mut group) = insertion.new_group.take() {
            let mut block = insertion
                .new_block
                .take()
                .expect("preallocated new group block");
            block.insert(id, coordinates);
            group.blocks.push(block);
            group.live = 1;
            self.positions
                .insert(insertion.signature, self.groups.len());
            self.groups.push(group);
        } else {
            let position = self.positions[&insertion.signature];
            let group = &mut self.groups[position];
            if let Some(block) = insertion.new_block.take() {
                group.blocks.push(block);
            }
            group
                .blocks
                .last_mut()
                .expect("reserved tail block")
                .insert(id, coordinates);
            group.live += 1; // bounded by checked global live + 1
        }
        self.live += 1; // checked by prepare before any retirement
    }

    #[cfg(test)]
    pub(super) fn ids(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self
            .groups
            .iter()
            .flat_map(|group| {
                group
                    .blocks
                    .iter()
                    .flat_map(|block| block.ids().iter().copied())
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    #[cfg(test)]
    pub(super) fn groups(&self) -> usize {
        self.groups.len()
    }

    #[cfg(test)]
    fn block_storage(&self) -> BlockStorage {
        let blocks = self.groups.iter().map(|group| group.blocks.len()).sum();
        BlockStorage {
            live_ids: self.live,
            blocks,
            id_slots: blocks * blocks::BLOCK_SIZE,
            block_capacity_bytes: self
                .groups
                .iter()
                .map(|group| group.blocks.capacity() * std::mem::size_of::<Block>())
                .sum(),
            envelope_capacity_bytes: self
                .groups
                .iter()
                .flat_map(|group| &group.blocks)
                .map(Block::envelope_capacity_bytes)
                .sum(),
        }
    }

    #[cfg(test)]
    fn record_group(&self, eligible: bool) {
        self.work.groups_visited.fetch_add(1, Ordering::Relaxed);
        self.work
            .groups_rejected
            .fetch_add(usize::from(!eligible), Ordering::Relaxed);
    }

    #[cfg(test)]
    fn record_block(&self, eligible: bool) {
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

#[cfg(test)]
mod tests;
