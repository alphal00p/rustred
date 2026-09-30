//! Immutable process-local lookup roots. Geometry pages survive index
//! compaction; a cut copies only changed live-bit paths and appended pages.
//! Index cohorts partition the ID range. Binary-size compaction bounds their
//! number logarithmically and reuses the existing native aggregate prefilter.
//! No cohort order or retirement is mathematical authority: every positive
//! is checked by the common verifier against its ID-bound immutable geometry.
use super::super::super::queue::{
    AggregateIndex, CompactDomain, CompactSummary, Coordinates, Entry, Probe, Query, Signature,
    Visit, rank_contains,
};
use super::super::store::{Hit, LookupCounters, Store, bucket_key};
use super::super::verify::{Container, QueryImage, Verified, VerifyCounters, verify};
use super::shared::{Copies, Pages};
use std::collections::HashMap;
use std::sync::Arc;

mod exact;

type Key = (u8, u32);

struct Layer<const N: usize> {
    first: u32,
    end: u32,
    // Sorted by digest then ID: collision confirmation and oldest admissible
    // exact ID require no hash-table allocation per singleton digest.
    exact: Vec<(u64, u32)>,
    buckets: HashMap<Key, AggregateIndex<N>>,
    orthants: HashMap<Key, u32>,
}

pub(in super::super) struct Image<const N: usize> {
    pub domains: Pages<CompactDomain<N>>,
    pub summaries: Pages<CompactSummary<N>>,
    live: Pages<bool>,
    pub quarantine: Pages<u64>,
    pub rescue_duplicates: bool,
    layers: Vec<Arc<Layer<N>>>,
}

impl<const N: usize> Image<N> {
    pub fn len(&self) -> usize {
        self.domains.len()
    }

    /// Logical retention charge, not a conservative allocator/RSS bound.
    /// A geometric carry can rebuild old index metadata: count that range,
    /// not just the newly appended IDs, before admitting a cut with readers.
    pub(super) fn refresh_charge(&self, additions: usize, retirements: usize) -> usize {
        if additions == 0 && retirements == 0 {
            return 0;
        }
        let end = self.len().saturating_add(additions);
        let mut first = self.len();
        for layer in self.layers.iter().rev() {
            if (layer.end - layer.first).ilog2() > end.saturating_sub(first).max(1).ilog2() {
                break;
            }
            first = layer.first as usize;
        }
        let row = std::mem::size_of::<CompactDomain<N>>()
            + std::mem::size_of::<CompactSummary<N>>()
            + 1024;
        end.saturating_sub(first)
            .saturating_mul(row)
            .saturating_add(retirements.saturating_mul(4096))
    }

    pub(super) fn bootstrap(
        store: &Store<N>,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        let mut result = Self {
            domains: Pages::default(),
            summaries: Pages::default(),
            live: Pages::default(),
            quarantine: Pages::default(),
            rescue_duplicates: store.rescue_duplicates,
            layers: Vec::new(),
        };
        result.append_geometry(store, false, work, checkpoint)?;
        let mut visited = 0usize;
        for bucket in &store.buckets {
            checkpoint()?;
            bucket.index.visit_live(|id| {
                if visited % 4096 == 0 {
                    checkpoint()?;
                }
                visited += 1;
                result.live.try_set(id as usize, true, work, checkpoint)
            })?;
        }
        if result.len() != 0 {
            result
                .layers
                .try_reserve(1)
                .map_err(|_| "snapshot layer allocation")?;
            result.layers.push(Arc::new(Layer::build(
                &result,
                0,
                result.len() as u32,
                &[],
                checkpoint,
            )?));
        }
        Ok(result)
    }

    fn append_geometry(
        &mut self,
        store: &Store<N>,
        initially_live: bool,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if self.len() > store.len() || store.len() > u32::MAX as usize {
            return Err("snapshot geometry watermark range");
        }
        for id in self.len()..store.len() {
            if id % 4096 == 0 {
                checkpoint()?;
            }
            self.domains.try_push(store.domains[id], work, checkpoint)?;
            self.summaries
                .try_push(store.summaries[id], work, checkpoint)?;
            self.live.try_push(initially_live, work, checkpoint)?;
        }
        // Quarantine can change only with a drained invocation-boundary
        // amendment, which discards all roots. Newly admitted IDs are clear.
        for id in self.quarantine.len()..store.quarantine.len() {
            self.quarantine
                .try_push(store.quarantine[id], work, checkpoint)?;
        }
        Ok(())
    }

    /// Build a candidate root off to the side. Failure never publishes a
    /// partially applied retirement or mutates a reader's historical root.
    pub(super) fn advance<'a>(
        &self,
        store: &Store<N>,
        updates: impl Iterator<Item = (u32, &'a [u32])>,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(self.layers.len() + 1)
            .map_err(|_| "snapshot layer allocation")?;
        layers.extend(self.layers.iter().cloned());
        let mut next = Self {
            domains: self.domains.clone(),
            summaries: self.summaries.clone(),
            live: self.live.clone(),
            quarantine: self.quarantine.clone(),
            rescue_duplicates: self.rescue_duplicates,
            layers,
        };
        next.append_geometry(store, true, work, checkpoint)?;
        let mut expected = self.len();
        for (id, retire) in updates {
            checkpoint()?;
            if id as usize != expected || expected >= next.len() {
                return Err("snapshot retirement journal ID order");
            }
            for (at, &old) in retire.iter().enumerate() {
                if at % 4096 == 0 {
                    checkpoint()?;
                }
                if old >= id || (at != 0 && retire[at - 1] >= old) {
                    return Err("snapshot retirement journal live ID");
                }
                // Distinct survivors of one cut can both contain the same
                // formerly-live ID. P2 computes their sets against one frozen
                // store; canonical P3 retires the intersection only once.
                next.live.try_set(old as usize, false, work, checkpoint)?;
            }
            expected += 1;
        }
        if expected != next.len() {
            return Err("snapshot retirement journal incomplete");
        }
        if next.len() != self.len() {
            let mut first = self.len() as u32;
            let end = next.len() as u32;
            // Geometric suffix compaction: floor(log2(size)) strictly falls
            // across layers. At most 32 layers for u32 IDs, even when cuts
            // arrive in adversarial, gradually decreasing sizes.
            let mut keep = next.layers.len();
            while keep != 0 && {
                let last = &next.layers[keep - 1];
                (last.end - last.first).ilog2() <= (end - first).ilog2()
            } {
                keep -= 1;
                first = next.layers[keep].first;
            }
            let layer = Layer::build(&next, first, end, &next.layers[keep..], checkpoint)?;
            next.layers.truncate(keep);
            next.layers.push(Arc::new(layer));
        }
        checkpoint()?;
        Ok(next)
    }

    fn quarantined(&self, id: u32) -> bool {
        self.quarantine
            .get(id as usize / 64)
            .is_some_and(|word| word >> (id % 64) & 1 != 0)
    }

    fn confirm(
        &self,
        id: u32,
        q: &QueryImage<N>,
        counters: &mut VerifyCounters,
    ) -> Result<Verified, String> {
        verify(
            Container::Snapshot {
                id,
                domains: &self.domains,
                published_len: self.len(),
            },
            q,
            counters,
        )
        .ok_or_else(|| format!("snapshot winner {id} failed independent verify"))
    }

    pub fn lookup(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        published_len: usize,
        counters: &mut LookupCounters,
        verify_counters: &mut VerifyCounters,
    ) -> Result<Option<(u32, Verified, Hit)>, String> {
        if published_len != self.len() {
            return Err("snapshot lookup watermark differs".into());
        }
        // Layers are disjoint ascending ID ranges; the first confirmed exact
        // ID is the globally oldest admissible one, retired or not.
        for layer in &self.layers {
            let first = layer
                .exact
                .partition_point(|&(digest, _)| digest < q.digest);
            for &(digest, id) in &layer.exact[first..] {
                if digest != q.digest {
                    break;
                }
                if self.domains.get(id as usize) == Some(&q.image) && !self.quarantined(id) {
                    let token = self.confirm(id, q, verify_counters)?;
                    counters.exact_hits += 1;
                    return Ok(Some((id, token, Hit::Exact)));
                }
            }
        }
        // Choose the dominant orthant before applying quarantine, exactly as
        // canonical Store does. A blocked dominant does not resurrect an old
        // retired orthant as a substitute; live fallback handles that query.
        let key = bucket_key(&q.image);
        let mut dominant = None;
        for layer in &self.layers {
            if let Some(&id) = layer.orthants.get(&key) {
                if dominant.is_none_or(|old| {
                    rank_contains(
                        self.domains[id as usize].rank(),
                        self.domains[old as usize].rank(),
                    )
                }) {
                    dominant = Some(id);
                }
            }
        }
        if let Some(id) = dominant {
            if !self.quarantined(id)
                && rank_contains(self.domains[id as usize].rank(), q.image.rank())
            {
                let token = self.confirm(id, q, verify_counters)?;
                counters.orthant_hits += 1;
                return Ok(Some((id, token, Hit::Orthant)));
            }
        }
        let probe = Probe::new(Coordinates::of(&q.core), query.word, query.lanes, true);
        for layer in &self.layers {
            let Some(index) = layer.buckets.get(&key) else {
                continue;
            };
            let found = index
                .find_controlled(
                    Signature::of(&q.core),
                    &probe,
                    0,
                    || Ok(()),
                    &mut Forward {
                        image: self,
                        query,
                        candidates: &mut counters.forward_candidates,
                        tests: &mut counters.forward_tests,
                    },
                )
                .map_err(str::to_owned)?;
            if let Some(id) = found {
                let id = id as u32;
                let token = self.confirm(id, q, verify_counters)?;
                counters.contained_hits += 1;
                return Ok(Some((id, token, Hit::Contained)));
            }
        }
        counters.misses += 1;
        Ok(None)
    }
}

#[cfg(test)]
mod tests;

impl<const N: usize> Layer<N> {
    fn build(
        image: &Image<N>,
        first: u32,
        end: u32,
        prior: &[Arc<Self>],
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        let mut result = Self {
            first,
            end,
            exact: exact::merge(image, first, end, prior, checkpoint)?,
            buckets: HashMap::new(),
            orthants: HashMap::new(),
        };
        for id in first..end {
            if id % 4096 == 0 {
                checkpoint()?;
            }
            let domain = &image.domains[id as usize];
            let key = bucket_key(domain);
            if domain.is_full_orthant()
                && result.orthants.get(&key).is_none_or(|&old| {
                    rank_contains(domain.rank(), image.domains[old as usize].rank())
                })
            {
                result
                    .orthants
                    .try_reserve(1)
                    .map_err(|_| "snapshot orthant allocation")?;
                result.orthants.insert(key, id);
            }
            if image.live.get(id as usize) != Some(&true) {
                continue;
            }
            // Reuse the native summary/index implementation. This is geometry
            // preprocessing, not symbolic coefficient evaluation or new CAS.
            let q = domain
                .try_native_summary()
                .map_err(|_| "snapshot native geometry invalid")?;
            let query = Query::new(q, domain.phase());
            let coordinates = Coordinates::of(&query.core);
            if !result.buckets.contains_key(&key) {
                result
                    .buckets
                    .try_reserve(1)
                    .map_err(|_| "snapshot bucket allocation")?;
                result.buckets.insert(key, AggregateIndex::default());
            }
            let index = result.buckets.get_mut(&key).expect("inserted bucket");
            let insertion =
                index.prepare_with(Signature::of(&query.core), coordinates, &mut *checkpoint)?;
            index.insert(
                insertion,
                Entry {
                    id: id as usize,
                    coordinates,
                    word: query.word,
                    lanes: query.lanes,
                },
            );
        }
        checkpoint()?;
        Ok(result)
    }
}

struct Forward<'a, const N: usize> {
    image: &'a Image<N>,
    query: &'a Query<N>,
    candidates: &'a mut u64,
    tests: &'a mut u64,
}
impl<const N: usize> Visit for Forward<'_, N> {
    fn rejected(&mut self, run: &[u32], _: u32) -> Result<(), &'static str> {
        *self.candidates += run.len() as u64;
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        *self.candidates += 1;
        *self.tests += 1;
        if self.image.live.get(id) != Some(&true) || self.image.quarantined(id as u32) {
            return Ok(false);
        }
        let domain = &self.image.domains[id];
        let (phase, owner) = self.query.bucket;
        Ok(domain.same_bucket(phase, owner)
            && self.image.summaries[id]
                .contains(&self.query.compact)
                .unwrap_or_else(|| domain.native_summary().contains(&self.query.core)))
    }
}
