//! Dependency edges as u32 CSR-by-target plus an append log. The CSR holds
//! the folded edges grouped by target (insertion order within a target); the
//! log holds the edges added since the last fold, in insertion order, chained
//! per target so a refresh can walk incoming lists without a fold. Only the
//! log keeps global insertion order, which the checkpoint writer needs for
//! the not-yet-persisted tail; a fold is therefore only requested after a
//! save persisted the whole log.
use std::ops::ControlFlow;

/// Empty log chain; node counts and log lengths stay strictly below it.
pub(super) const NONE: u32 = u32::MAX;

/// Longest log a u32 chain link can address.
const LOG_CAP: usize = NONE as usize - 1;

/// Folded edges, incoming by target. `offsets` covers the node count at the
/// last fold (plus one); later nodes have no folded edges yet.
#[derive(Default)]
struct Csr {
    offsets: Vec<u64>,
    sources: Vec<u32>,
}
impl Csr {
    fn incoming(&self, target: usize) -> &[u32] {
        match (self.offsets.get(target), self.offsets.get(target + 1)) {
            (Some(&start), Some(&end)) => &self.sources[start as usize..end as usize],
            _ => &[],
        }
    }
    fn targets(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }
}

/// Edges since the last fold, in insertion order, linked by target (newest
/// first): 12 B per edge plus 4 B per node for the heads, which `grow` keeps
/// at one entry per node.
#[derive(Default)]
struct EdgeLog {
    pairs: Vec<(u32, u32)>,
    heads: Vec<u32>,
    next: Vec<u32>,
}

/// Sources of one target's log chain.
struct Chain<'a> {
    log: &'a EdgeLog,
    at: u32,
}
impl Iterator for Chain<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.at == NONE {
            return None;
        }
        let index = self.at as usize;
        self.at = self.log.next[index];
        Some(self.log.pairs[index].0)
    }
}

#[derive(Default)]
pub(super) struct Edges {
    csr: Csr,
    log: EdgeLog,
}

/// Targets per bucket of the cache-local counting sort: a bucket's offsets
/// (256 KiB) and its slice of the source array stay cache resident.
const BUCKET_SHIFT: u32 = 15;

/// Stable partition of `pairs` by target bucket (insertion order kept within
/// a bucket), so the counting and scatter passes below touch one bucket's
/// offsets and sources at a time instead of the whole arrays at random.
fn partition(nodes: usize, pairs: &[(u32, u32)]) -> Option<Vec<(u32, u32)>> {
    // Bucket `b` counts into `cursors[b + 1]`; the inclusive prefix sum then
    // leaves the start of bucket `b` in `cursors[b]`.
    let mut cursors = vec![0usize; (nodes >> BUCKET_SHIFT) + 2];
    for &(_, target) in pairs {
        cursors[(target >> BUCKET_SHIFT) as usize + 1] += 1;
    }
    for bucket in 1..cursors.len() {
        cursors[bucket] += cursors[bucket - 1];
    }
    let mut partitioned = Vec::new();
    partitioned.try_reserve_exact(pairs.len()).ok()?;
    partitioned.resize(pairs.len(), (0u32, 0u32));
    for &pair in pairs {
        let cursor = &mut cursors[(pair.1 >> BUCKET_SHIFT) as usize];
        partitioned[*cursor] = pair;
        *cursor += 1;
    }
    Some(partitioned)
}

/// Counting sort of `old` plus `extra` (insertion order) into a CSR over
/// `nodes` targets; within a target the old sources precede the extra ones,
/// both in their original order. Peak: the old and the new source arrays
/// plus one bucket-partitioned copy of `extra`.
fn build(nodes: usize, old: &Csr, extra: &[(u32, u32)]) -> Option<Csr> {
    let total = old.sources.len().checked_add(extra.len())?;
    let extra = partition(nodes, extra)?;
    let mut offsets = Vec::new();
    offsets.try_reserve_exact(nodes.checked_add(1)?).ok()?;
    offsets.extend((0..nodes).map(|target| old.incoming(target).len() as u64));
    offsets.push(0);
    for &(_, target) in &extra {
        offsets[target as usize] += 1;
    }
    let mut running = 0u64;
    for offset in &mut offsets[..nodes] {
        let count = *offset;
        *offset = running;
        running += count;
    }
    offsets[nodes] = running;
    let mut sources = Vec::new();
    sources.try_reserve_exact(total).ok()?;
    sources.resize(total, 0u32);
    // `offsets[target]` serves as the insertion cursor, ending at the start
    // of `target + 1`; one shift afterwards restores the starts.
    for (target, offset) in offsets[..nodes].iter_mut().enumerate() {
        let incoming = old.incoming(target);
        let start = *offset as usize;
        sources[start..start + incoming.len()].copy_from_slice(incoming);
        *offset += incoming.len() as u64;
    }
    for &(source, target) in &extra {
        let cursor = &mut offsets[target as usize];
        sources[*cursor as usize] = source;
        *cursor += 1;
    }
    offsets.copy_within(..nodes, 1);
    offsets[0] = 0;
    Some(Csr { offsets, sources })
}

impl Edges {
    /// Restore from a repeatable immutable edge iterator with no temporary
    /// pair/partition arrays. Both passes keep the source order within each
    /// target. Allocations are only the final CSR and normal empty-log heads.
    pub fn from_iter(
        nodes: usize,
        pairs: impl Iterator<Item = (u32, u32)> + Clone,
    ) -> Result<Self, String> {
        if nodes >= NONE as usize {
            return Err("invalid checkpoint dependency inventory".into());
        }
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(nodes + 1)
            .map_err(|_| "dependency restore allocation")?;
        offsets.resize(nodes + 1, 0u64);
        let mut total = 0usize;
        let mut first_hash = blake3::Hasher::new();
        for (source, target) in pairs.clone() {
            if source as usize >= nodes || target as usize >= nodes {
                return Err("invalid checkpoint dependency edge".into());
            }
            total = total
                .checked_add(1)
                .ok_or("dependency edge count overflow")?;
            offsets[target as usize + 1] += 1;
            first_hash.update(&source.to_le_bytes());
            first_hash.update(&target.to_le_bytes());
        }
        for target in 1..=nodes {
            offsets[target] += offsets[target - 1];
        }
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(total)
            .map_err(|_| "dependency restore allocation")?;
        sources.resize(total, 0u32);
        let mut copied = 0usize;
        let mut second_hash = blake3::Hasher::new();
        for (source, target) in pairs {
            if source as usize >= nodes || target as usize >= nodes {
                return Err("invalid checkpoint dependency edge".into());
            }
            let cursor = &mut offsets[target as usize];
            let slot = sources
                .get_mut(*cursor as usize)
                .ok_or("dependency iterator changed")?;
            *slot = source;
            *cursor += 1;
            copied += 1;
            second_hash.update(&source.to_le_bytes());
            second_hash.update(&target.to_le_bytes());
        }
        if copied != total || first_hash.finalize() != second_hash.finalize() {
            return Err("dependency iterator changed".into());
        }
        offsets.copy_within(..nodes, 1);
        offsets[0] = 0;
        let mut edges = Self {
            csr: Csr { offsets, sources },
            log: EdgeLog::default(),
        };
        edges
            .grow(nodes)
            .map_err(|()| "dependency restore allocation")?;
        Ok(edges)
    }

    /// Rebuild the folded graph from persisted (source, target) pairs; every
    /// endpoint must name one of `nodes` nodes.
    pub fn from_pairs(nodes: usize, pairs: &[(u32, u32)]) -> Result<Self, String> {
        if nodes >= NONE as usize {
            return Err("invalid checkpoint dependency inventory".into());
        }
        if pairs
            .iter()
            .any(|&(source, target)| source as usize >= nodes || target as usize >= nodes)
        {
            return Err("invalid checkpoint dependency edge".into());
        }
        let csr = build(nodes, &Csr::default(), pairs).ok_or("dependency restore allocation")?;
        let mut edges = Self {
            csr,
            log: EdgeLog::default(),
        };
        edges
            .grow(nodes)
            .map_err(|()| "dependency restore allocation")?;
        Ok(edges)
    }

    /// Extend the per-node log heads to `nodes` entries.
    pub fn grow(&mut self, nodes: usize) -> Result<(), ()> {
        let extra = nodes.saturating_sub(self.log.heads.len());
        if nodes >= NONE as usize || self.log.heads.try_reserve(extra).is_err() {
            return Err(());
        }
        self.log.heads.resize(nodes, NONE);
        Ok(())
    }

    /// Append one edge; endpoints were validated against the node count.
    pub fn push(&mut self, source: u32, target: u32) -> Result<(), ()> {
        self.push_within(source, target, LOG_CAP)
    }

    /// Reserve the immediately appendable portion of a batch without changing
    /// any logical edges. A later log-cap fold may need a separate allocation.
    pub fn reserve_append(&mut self, additional: usize) -> Result<(), ()> {
        let immediate = additional.min(LOG_CAP.saturating_sub(self.log.pairs.len()));
        self.log.pairs.try_reserve(immediate).map_err(|_| ())?;
        self.log.next.try_reserve(immediate).map_err(|_| ())
    }

    /// Append one source's already validated targets in order. Reserve once
    /// per log segment, not once per edge. Fold at the same boundaries as the
    /// scalar path so checkpoint-tail ordering is unchanged.
    pub fn push_source(&mut self, source: u32, targets: &[u32]) -> Result<(), ()> {
        self.push_source_within(source, targets, LOG_CAP)
    }

    fn push_source_within(&mut self, source: u32, targets: &[u32], cap: usize) -> Result<(), ()> {
        self.push_source_with_checkpoint(source, targets, cap, || Ok(()))
    }

    fn push_source_with_checkpoint(
        &mut self,
        source: u32,
        mut targets: &[u32],
        cap: usize,
        mut checkpoint: impl FnMut() -> Result<(), ()>,
    ) -> Result<(), ()> {
        if cap == 0 || cap > LOG_CAP {
            return Err(());
        }
        while !targets.is_empty() {
            checkpoint()?;
            if self.log.pairs.len() >= cap {
                self.fold(self.log.heads.len())?;
            }
            let take = targets.len().min(cap - self.log.pairs.len());
            self.log.pairs.try_reserve(take).map_err(|_| ())?;
            self.log.next.try_reserve(take).map_err(|_| ())?;
            for &target in &targets[..take] {
                let index = self.log.pairs.len();
                let head = &mut self.log.heads[target as usize];
                self.log.next.push(*head);
                *head = index as u32;
                self.log.pairs.push((source, target));
            }
            targets = &targets[take..];
        }
        Ok(())
    }

    /// `push` with a log of at most `cap` edges. A full log folds first
    /// instead of refusing the edge: without a checkpoint store nothing else
    /// folds, and with one the next save re-tiles the edges from zero (its
    /// retained tiling then ends inside the folded prefix). Only a failed
    /// fold allocation refuses the edge.
    fn push_within(&mut self, source: u32, target: u32, cap: usize) -> Result<(), ()> {
        if self.log.pairs.len() >= cap {
            self.fold(self.log.heads.len())?;
        }
        let index = self.log.pairs.len();
        if self.log.pairs.try_reserve(1).is_err() || self.log.next.try_reserve(1).is_err() {
            return Err(());
        }
        let head = &mut self.log.heads[target as usize];
        self.log.next.push(*head);
        *head = index as u32;
        self.log.pairs.push((source, target));
        Ok(())
    }

    /// Merge the log into the CSR (counting sort, O(edges + nodes), peak two
    /// source arrays). A failed allocation keeps the log: nothing is lost.
    pub fn fold(&mut self, nodes: usize) -> Result<(), ()> {
        if self.log.pairs.is_empty() {
            return Ok(());
        }
        let csr = build(nodes, &self.csr, &self.log.pairs).ok_or(())?;
        self.csr = csr;
        self.log.pairs = Vec::new();
        self.log.next = Vec::new();
        self.log.heads.fill(NONE);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.csr.sources.len() + self.log.pairs.len()
    }

    /// Edges whose insertion order was given up by a fold.
    pub fn folded(&self) -> usize {
        self.csr.sources.len()
    }

    pub fn log_len(&self) -> usize {
        self.log.pairs.len()
    }

    /// Sources of every edge into `target`: folded ones, then the log chain.
    pub fn incoming(&self, target: usize) -> impl Iterator<Item = u32> + '_ {
        self.csr.incoming(target).iter().copied().chain(Chain {
            log: &self.log,
            at: self.log.heads.get(target).copied().unwrap_or(NONE),
        })
    }

    /// Every edge once: folded edges grouped by target, then the log in
    /// insertion order. Callers must not depend on this order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        (0..self.csr.targets())
            .flat_map(move |target| {
                self.csr
                    .incoming(target)
                    .iter()
                    .map(move |&source| (source, target as u32))
            })
            .chain(self.log.pairs.iter().copied())
    }

    pub fn try_for_each<B>(
        &self,
        mut visit: impl FnMut(u32, u32) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for target in 0..self.csr.targets() {
            for &source in self.csr.incoming(target) {
                visit(source, target as u32)?;
            }
        }
        for &(source, target) in &self.log.pairs {
            visit(source, target)?;
        }
        ControlFlow::Continue(())
    }

    /// The checkpoint segment `[first, first + count)` of the virtual edge
    /// sequence (folded edges in `iter` order, then the log). Only a full
    /// re-tile (`first == 0`) or a range inside the log is well defined: the
    /// insertion order of folded edges is gone.
    pub fn segment(
        &self,
        first: usize,
        count: usize,
    ) -> Result<Box<dyn Iterator<Item = (u32, u32)> + '_>, String> {
        if first.checked_add(count).is_none_or(|end| end > self.len()) {
            return Err("dependency edge segment outside the graph".into());
        }
        if first == 0 {
            return Ok(Box::new(self.iter().take(count)));
        }
        let Some(start) = first.checked_sub(self.folded()) else {
            return Err("dependency edge segment starts inside the folded prefix".into());
        };
        Ok(Box::new(
            self.log.pairs[start..start + count].iter().copied(),
        ))
    }

    /// Logical capacities in bytes (no allocator overhead).
    pub fn storage_bytes(&self) -> usize {
        [
            self.csr.offsets.capacity() * size_of::<u64>(),
            self.csr.sources.capacity() * size_of::<u32>(),
            self.log.pairs.capacity() * size_of::<(u32, u32)>(),
            self.log.heads.capacity() * size_of::<u32>(),
            self.log.next.capacity() * size_of::<u32>(),
        ]
        .into_iter()
        .fold(0usize, usize::saturating_add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pairs over `nodes`, with repeated targets so the order
    /// within a target is observable.
    fn random_pairs(nodes: usize, count: usize, seed: u64) -> Vec<(u32, u32)> {
        let mut state = seed;
        (0..count)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let word = state >> 11;
                let source = (word % nodes as u64) as u32;
                let target = ((word >> 26) % nodes as u64) as u32;
                (source, target)
            })
            .collect()
    }

    #[test]
    fn csr_keeps_every_edge_and_its_order_across_buckets() {
        // Three full target buckets and a partial one. The first part only
        // touches the lower half, so its CSR covers fewer targets than the
        // fold; within a target, older sources precede the log's.
        let nodes = (3 << BUCKET_SHIFT) + 5;
        let half = nodes / 2;
        let (first, second): (Vec<_>, Vec<_>) = random_pairs(nodes, 200_000, 7)
            .into_iter()
            .partition(|&(source, target)| (source as usize) < half && (target as usize) < half);
        let all: Vec<_> = first.iter().chain(&second).copied().collect();
        let mut expected = vec![Vec::new(); nodes];
        for &(source, target) in &all {
            expected[target as usize].push(source);
        }
        assert!(expected.iter().filter(|sources| sources.len() > 1).count() > 1000);
        let check = |edges: &Edges| {
            assert_eq!((edges.len(), edges.log_len()), (all.len(), 0));
            for (target, sources) in expected.iter().enumerate() {
                assert_eq!(edges.csr.incoming(target), sources.as_slice(), "{target}");
            }
            let mut listed: Vec<_> = edges.iter().collect();
            let mut original = all.clone();
            listed.sort_unstable();
            original.sort_unstable();
            assert_eq!(listed, original);
        };
        // Restore: every persisted pair at once.
        check(&Edges::from_pairs(nodes, &all).unwrap());
        // Fold: a CSR over the lower half, then a log across every bucket.
        let mut folded = Edges::from_pairs(half, &first).unwrap();
        folded.grow(nodes).unwrap();
        for &(source, target) in &second {
            folded.push(source, target).unwrap();
        }
        folded.fold(nodes).unwrap();
        check(&folded);
    }

    #[test]
    fn a_full_log_folds_instead_of_refusing_an_edge() {
        let nodes = 7;
        let pairs = random_pairs(nodes, 100, 11);
        let mut edges = Edges::default();
        edges.grow(nodes).unwrap();
        for (count, &(source, target)) in pairs.iter().enumerate() {
            edges.push_within(source, target, 8).unwrap();
            assert_eq!(edges.len(), count + 1);
            assert!(edges.log_len() <= 8);
        }
        assert_eq!(edges.folded(), 96, "each full log of 8 edges folded");
        edges.fold(nodes).unwrap();
        for target in 0..nodes {
            let expected: Vec<_> = pairs
                .iter()
                .filter(|pair| pair.1 as usize == target)
                .map(|pair| pair.0)
                .collect();
            assert_eq!(edges.csr.incoming(target), expected.as_slice(), "{target}");
        }
    }

    #[test]
    fn source_batches_preserve_scalar_fold_and_checkpoint_boundaries() {
        let nodes = 7;
        for cap in [1, 2, 5, 8] {
            let mut scalar = Edges::default();
            let mut batch = Edges::default();
            scalar.grow(nodes).unwrap();
            batch.grow(nodes).unwrap();
            for source in 0..nodes as u32 {
                for count in [0, 1, cap, cap + 1, 3 * cap + 2] {
                    let targets: Vec<_> = (0..count)
                        .map(|i| ((i + source as usize) % nodes) as u32)
                        .collect();
                    for &target in &targets {
                        scalar.push_within(source, target, cap).unwrap();
                    }
                    batch.push_source_within(source, &targets, cap).unwrap();
                    assert_eq!(scalar.len(), batch.len());
                    assert_eq!(scalar.folded(), batch.folded());
                    assert_eq!(scalar.log.pairs, batch.log.pairs);
                    assert_eq!(scalar.log.next, batch.log.next);
                    assert_eq!(scalar.log.heads, batch.log.heads);
                    assert_eq!(
                        scalar.iter().collect::<Vec<_>>(),
                        batch.iter().collect::<Vec<_>>()
                    );
                    for target in 0..nodes {
                        assert_eq!(
                            scalar.incoming(target).collect::<Vec<_>>(),
                            batch.incoming(target).collect::<Vec<_>>()
                        );
                    }
                    for first in [0, scalar.folded(), scalar.len()] {
                        let count = scalar.len() - first;
                        assert_eq!(
                            scalar.segment(first, count).unwrap().collect::<Vec<_>>(),
                            batch.segment(first, count).unwrap().collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn batch_reservation_changes_capacity_not_logical_state() {
        let mut edges = Edges::default();
        edges.grow(3).unwrap();
        edges.push(0, 1).unwrap();
        let before: Vec<_> = edges.iter().collect();
        edges.reserve_append(100).unwrap();
        assert_eq!(edges.iter().collect::<Vec<_>>(), before);
        assert!(edges.log.pairs.capacity() >= 101);
        assert!(edges.log.next.capacity() >= 101);
        assert_eq!(edges.folded(), 0);
        assert_eq!(edges.log_len(), 1);
        assert!(edges.push_source_within(0, &[2], 0).is_err());
        assert_eq!(edges.iter().collect::<Vec<_>>(), before);
    }

    #[test]
    fn failed_later_batch_chunk_retains_exact_successful_prefix() {
        let mut edges = Edges::default();
        edges.grow(4).unwrap();
        let mut checkpoints = 0;
        let result = edges.push_source_with_checkpoint(0, &[1, 2, 3], 2, || {
            checkpoints += 1;
            if checkpoints == 2 { Err(()) } else { Ok(()) }
        });
        assert!(result.is_err());
        assert_eq!(checkpoints, 2);
        assert_eq!(edges.iter().collect::<Vec<_>>(), [(0, 1), (0, 2)]);
        assert_eq!(edges.folded(), 0);
        assert_eq!(edges.log_len(), 2);
        // A retry starting after the committed prefix can still fold/append.
        edges.push_source_within(0, &[3], 2).unwrap();
        assert_eq!(edges.iter().collect::<Vec<_>>(), [(0, 1), (0, 2), (0, 3)]);
        assert_eq!(edges.folded(), 2);
    }
}
