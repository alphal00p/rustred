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
/// first): 12 B per edge plus 4 B per node for the heads.
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

/// Counting sort of `old` plus `extra` (insertion order) into a CSR over
/// `nodes` targets; within a target the old sources precede the extra ones,
/// both in their original order. Peak: the old and the new source arrays.
fn build(nodes: usize, old: &Csr, extra: &[(u32, u32)]) -> Option<Csr> {
    let total = old.sources.len().checked_add(extra.len())?;
    let mut offsets = Vec::new();
    offsets.try_reserve_exact(nodes.checked_add(1)?).ok()?;
    offsets.extend((0..nodes).map(|target| old.incoming(target).len() as u64));
    offsets.push(0);
    for &(_, target) in extra {
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
    for &(source, target) in extra {
        let cursor = &mut offsets[target as usize];
        sources[*cursor as usize] = source;
        *cursor += 1;
    }
    offsets.copy_within(..nodes, 1);
    offsets[0] = 0;
    Some(Csr { offsets, sources })
}

impl Edges {
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
        let index = self.log.pairs.len();
        if index >= NONE as usize - 1
            || self.log.pairs.try_reserve(1).is_err()
            || self.log.next.try_reserve(1).is_err()
        {
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
