//! The canonical store of the epoch engine: the append-only `CompactDomain`
//! arena, the per-ID immutable summaries the reused kernel-lane index needs,
//! the digest-sharded exact index with overflow (F3), and per-bucket lookup
//! state (the ID-ordered SoA `AggregateIndex` of the kernel lane, reused as
//! code and mutated by P3, plus the dominant full orthant).
//!
//! S2 canonical resolution (protocol §17.2 S2: "canonical in-merge
//! resolution over the kernel lane's ID-ordered SoA index") is the legacy
//! index semantics: exact image, else the bucket's dominant full orthant,
//! else the minimum live ID of the index whose summary contains the query.
//! Every positive then passes `verify` (the authority); the index predicate
//! is a prefilter. The result is a deterministic function of the store.
use super::super::queue::{
    AggregateIndex, CompactDomain, CompactSummary, Coordinates, Entry, Insertion, Phase, Probe,
    Query, Retire, Signature, Stored, Visit, rank_contains,
};
use super::verify::{Container, QueryImage, Verified, VerifyCounters, verify};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Number of exact-index shards (a resize touches one shard, §3.2).
pub(super) const EXACT_SHARDS: usize = 4096;

/// The digest is uniformly distributed already, but a shard holds only
/// digests with equal top 12 bits (`shard_of`), and hashbrown takes its
/// 7-bit control tag from the top bits of the hash: an identity hash would
/// give every key of a shard the same tag. One multiply spreads every
/// digest bit into the top bits (performance only; no iteration order of
/// these maps reaches a result).
#[derive(Default)]
pub(super) struct IdentityHasher(u64);
impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (value ^ (value >> 29)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
}
type DigestMap<V> = HashMap<u64, V, BuildHasherDefault<IdentityHasher>>;

/// Exact-duplicate index, 4,096 digest shards plus an overflow map. A digest
/// hit is a candidate only: the stored image is compared byte for byte.
pub(super) struct ExactIndex {
    shards: Vec<DigestMap<u32>>,
    overflow: DigestMap<Vec<u32>>,
    entries: u64,
}

fn shard_of(digest: u64) -> usize {
    (digest >> 52) as usize % EXACT_SHARDS
}

impl ExactIndex {
    pub fn new() -> Self {
        Self {
            shards: (0..EXACT_SHARDS).map(|_| DigestMap::default()).collect(),
            overflow: DigestMap::default(),
            entries: 0,
        }
    }
    pub fn len(&self) -> u64 {
        self.entries
    }
    pub fn overflow_len(&self) -> usize {
        self.overflow.values().map(Vec::len).sum()
    }
    pub fn get<const N: usize>(
        &self,
        digest: u64,
        image: &CompactDomain<N>,
        domains: &[CompactDomain<N>],
    ) -> Option<u32> {
        self.get_admissible(digest, image, domains, |_| true)
    }

    pub fn get_admissible<const N: usize>(
        &self,
        digest: u64,
        image: &CompactDomain<N>,
        domains: &[CompactDomain<N>],
        admissible: impl Fn(u32) -> bool,
    ) -> Option<u32> {
        let &id = self.shards[shard_of(digest)].get(&digest)?;
        if domains[id as usize] == *image && admissible(id) {
            return Some(id);
        }
        self.overflow
            .get(&digest)?
            .iter()
            .copied()
            .find(|&id| domains[id as usize] == *image && admissible(id))
    }
    fn primary_taken(&self, digest: u64) -> bool {
        self.shards[shard_of(digest)].contains_key(&digest)
    }
    /// Reserve for `digests` (the survivors of one merge, distinct images):
    /// every later `insert` of them cannot fail.
    pub fn try_reserve(&mut self, digests: &[u64]) -> Result<(), &'static str> {
        let mut per_shard: HashMap<usize, usize> = HashMap::new();
        let mut seen: HashMap<u64, ()> = HashMap::new();
        let mut overflow: HashMap<u64, usize> = HashMap::new();
        for &digest in digests {
            if self.primary_taken(digest) || seen.contains_key(&digest) {
                *overflow.entry(digest).or_default() += 1;
            } else {
                *per_shard.entry(shard_of(digest)).or_default() += 1;
                seen.insert(digest, ());
            }
        }
        for (shard, count) in per_shard {
            self.shards[shard]
                .try_reserve(count)
                .map_err(|_| "exact index allocation")?;
        }
        if !overflow.is_empty() {
            self.overflow
                .try_reserve(overflow.len())
                .map_err(|_| "exact index allocation")?;
            for (digest, count) in overflow {
                self.overflow
                    .entry(digest)
                    .or_default()
                    .try_reserve(count)
                    .map_err(|_| "exact index allocation")?;
            }
        }
        Ok(())
    }

    /// Bounded restore insertion: avoid constructing three temporary maps
    /// for each individual image while decoding a section as a stream.
    pub fn try_reserve_one(&mut self, digest: u64) -> Result<(), &'static str> {
        if self.primary_taken(digest) {
            self.overflow
                .try_reserve(1)
                .map_err(|_| "exact index allocation")?;
            self.overflow
                .entry(digest)
                .or_default()
                .try_reserve(1)
                .map_err(|_| "exact index allocation")
        } else {
            self.shards[shard_of(digest)]
                .try_reserve(1)
                .map_err(|_| "exact index allocation")
        }
    }
    /// Insert `id` under `digest`. The caller (`Store::push`) has refused an
    /// equal image already (E3: exact uniqueness of images).
    pub fn insert(&mut self, digest: u64, id: u32) {
        let shard = &mut self.shards[shard_of(digest)];
        if let std::collections::hash_map::Entry::Vacant(slot) = shard.entry(digest) {
            slot.insert(id);
        } else {
            self.overflow.entry(digest).or_default().push(id);
        }
        self.entries += 1;
    }
}

/// One (phase, owner) bucket, interned in ID order of first appearance (A3).
pub(super) struct Bucket<const N: usize> {
    pub index: AggregateIndex<N>,
    /// The dominant admitted full orthant (largest rank; None rank dominates).
    pub orthant: Option<u32>,
}

/// How a lookup resolved (counters and records).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Exact,
    Orthant,
    Contained,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LookupCounters {
    pub exact_hits: u64,
    pub orthant_hits: u64,
    pub contained_hits: u64,
    pub misses: u64,
    /// Forward candidates handed to the index predicate (prefilter-rejected
    /// runs included). Includes inspector work when private S4 lookup is used;
    /// coordinator verification is counted separately in VerifyCounters.
    pub forward_candidates: u64,
    pub forward_tests: u64,
    pub reverse_candidates: u64,
    pub reverse_tests: u64,
}

impl LookupCounters {
    pub fn add(&mut self, other: &Self) {
        self.exact_hits += other.exact_hits;
        self.orthant_hits += other.orthant_hits;
        self.contained_hits += other.contained_hits;
        self.misses += other.misses;
        self.forward_candidates += other.forward_candidates;
        self.forward_tests += other.forward_tests;
        self.reverse_candidates += other.reverse_candidates;
        self.reverse_tests += other.reverse_tests;
    }
}

/// Initial admission's prepared index mutation. A new bucket must travel
/// with its token: `prepare` reserves storage on the index as well as in
/// the owned insertion. Between preparation and consumption, only the
/// corresponding `push` may mutate this store.
pub(super) struct InitialInsertion<const N: usize> {
    id: u32,
    bucket: u32,
    key: (u8, u32),
    new_bucket: Option<Bucket<N>>,
    insertion: Insertion<N>,
}

pub(super) struct Store<const N: usize> {
    pub domains: Vec<CompactDomain<N>>,
    pub summaries: Vec<CompactSummary<N>>,
    pub exact: ExactIndex,
    pub buckets: Vec<Bucket<N>>,
    pub bucket_of: HashMap<(u8, u32), u32>,
    pub max_finite_rank: Option<u32>,
    pub unbounded_rank_domains: u64,
    pub encountered_rank: super::super::rank_telemetry::RankCensus,
    /// Resume-boundary lookup exclusions. Historical edges/records are untouched.
    pub quarantine: Vec<u64>,
    /// Explicitly enabled only by an authenticated rescue amendment chain.
    pub rescue_duplicates: bool,
}

struct Forward<'a, const N: usize> {
    stored: Stored<'a, N>,
    query: &'a Query<N>,
    candidates: &'a mut u64,
    tests: &'a mut u64,
}
impl<const N: usize> Visit for Forward<'_, N> {
    fn rejected(&mut self, run: &[u32], _word: u32) -> Result<(), &'static str> {
        *self.candidates += run.len() as u64;
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        *self.candidates += 1;
        *self.tests += 1;
        Ok(self.stored.contains(id, self.query))
    }
}

struct Reverse<'a, const N: usize> {
    stored: Stored<'a, N>,
    query: &'a Query<N>,
    candidates: &'a mut u64,
    tests: &'a mut u64,
}
impl<const N: usize> Visit for Reverse<'_, N> {
    fn rejected(&mut self, run: &[u32], _word: u32) -> Result<(), &'static str> {
        *self.candidates += run.len() as u64;
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        *self.candidates += 1;
        *self.tests += 1;
        Ok(!self
            .stored
            .quarantine
            .get(id / 64)
            .is_some_and(|w| w >> (id % 64) & 1 != 0)
            && self.stored.contained_by(id, self.query))
    }
}

/// P3 retirement by a P2-prepared set (ascending old IDs).
struct PreparedRetire<'a> {
    set: &'a [u32],
    retired: u64,
}
impl Retire for PreparedRetire<'_> {
    fn rejected(&mut self, _run: &[u32], _word: u32) {}
    fn test(&mut self, id: usize) -> bool {
        let hit = u32::try_from(id).is_ok_and(|id| self.set.binary_search(&id).is_ok());
        self.retired += u64::from(hit);
        hit
    }
}

pub(super) fn phase_code(phase: Phase) -> u8 {
    match phase {
        Phase::Apply => 0,
        Phase::Route => 1,
    }
}

pub(super) fn bucket_key<const N: usize>(image: &CompactDomain<N>) -> (u8, u32) {
    let owner = image
        .owner()
        .iter()
        .enumerate()
        .fold(0u32, |bits, (axis, &on)| bits | (u32::from(on) << axis));
    (phase_code(image.phase()), owner)
}

impl<const N: usize> Store<N> {
    pub fn new() -> Self {
        Self {
            domains: Vec::new(),
            summaries: Vec::new(),
            exact: ExactIndex::new(),
            buckets: Vec::new(),
            bucket_of: HashMap::new(),
            max_finite_rank: None,
            unbounded_rank_domains: 0,
            encountered_rank: Default::default(),
            quarantine: Vec::new(),
            rescue_duplicates: false,
        }
    }

    pub fn len(&self) -> usize {
        self.domains.len()
    }

    pub fn is_quarantined(&self, id: u32) -> bool {
        self.quarantine
            .get(id as usize / 64)
            .is_some_and(|w| w >> (id % 64) & 1 != 0)
    }

    pub fn install_quarantine(&mut self, bits: Vec<u64>) -> Result<(), &'static str> {
        if !self.rescue_duplicates
            || bits.len() != self.len().div_ceil(64)
            || self.len() % 64 != 0 && bits.last().is_some_and(|w| w >> (self.len() % 64) != 0)
        {
            return Err("rescue quarantine mode, count or padding");
        }
        self.quarantine = bits;
        Ok(())
    }

    fn stored(&self) -> Stored<'_, N> {
        Stored {
            domains: &self.domains,
            summaries: &self.summaries,
            quarantine: &self.quarantine,
        }
    }

    /// Collision-confirmed exact lookup, including retired images. A positive
    /// still passes the same independent containment verifier as any other hit.
    /// A negative says nothing about containment by a different image.
    pub fn lookup_exact(
        &self,
        q: &QueryImage<N>,
        published_len: usize,
        counters: &mut LookupCounters,
        verify_counters: &mut VerifyCounters,
    ) -> Result<Option<(u32, Verified, Hit)>, String> {
        if let Some(id) = self
            .exact
            .get_admissible(q.digest, &q.image, &self.domains, |id| {
                (id as usize) < published_len && !self.is_quarantined(id)
            })
        {
            let token = verify(
                Container::Stored {
                    id,
                    domains: &self.domains,
                    published_len,
                },
                q,
                verify_counters,
            )
            .ok_or_else(|| format!("exact hit {id} failed verify"))?;
            counters.exact_hits += 1;
            return Ok(Some((id, token, Hit::Exact)));
        }
        Ok(None)
    }

    /// Canonical S2 resolution of `q` against the store (the snapshot S_k:
    /// every ID < `published_len`): exact, orthant, minimum live ID of the
    /// ID-ordered index. The winner is verified; an index winner that fails
    /// `verify` is an engine inconsistency (Err, C5).
    pub fn lookup(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        published_len: usize,
        counters: &mut LookupCounters,
        verify_counters: &mut VerifyCounters,
    ) -> Result<Option<(u32, Verified, Hit)>, String> {
        self.lookup_controlled(
            q,
            query,
            published_len,
            counters,
            verify_counters,
            || Ok(()),
        )
    }

    /// The identical canonical lookup with cooperative preparation checkpoints.
    /// The checkpoint neither charges counters nor changes winner selection.
    pub fn lookup_controlled(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        published_len: usize,
        counters: &mut LookupCounters,
        verify_counters: &mut VerifyCounters,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Option<(u32, Verified, Hit)>, String> {
        self.lookup_suffix_controlled(
            q,
            query,
            published_len,
            0,
            counters,
            verify_counters,
            checkpoint,
        )
    }

    /// The caller may skip an aggregate prefix disproved by a native negative
    /// in this invocation. Old geometry never changes, live IDs only retire,
    /// and quarantine cannot change during the native controller session.
    /// Exact and dominant-orthant lookup remain global and keep their priority;
    /// every positive is verified as before. An unbound caller must pass zero.
    pub fn lookup_suffix_controlled(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        published_len: usize,
        first_id: usize,
        counters: &mut LookupCounters,
        verify_counters: &mut VerifyCounters,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Option<(u32, Verified, Hit)>, String> {
        checkpoint().map_err(str::to_owned)?;
        if first_id > published_len {
            return Err("lookup prefix beyond published length".into());
        }
        let container = |id: u32| Container::Stored {
            id,
            domains: &self.domains,
            published_len,
        };
        if let Some(hit) = self.lookup_exact(q, published_len, counters, verify_counters)? {
            return Ok(Some(hit));
        }
        let Some(&bucket) = self.bucket_of.get(&bucket_key(&q.image)) else {
            counters.misses += 1;
            return Ok(None);
        };
        let bucket = &self.buckets[bucket as usize];
        if let Some(orthant) = bucket.orthant {
            let stored = &self.domains[orthant as usize];
            // A1: the stored image's bucket, full-orthant shape and rank.
            if bucket_key(stored) == bucket_key(&q.image)
                && stored.is_full_orthant()
                && rank_contains(stored.rank(), q.image.rank())
                && (orthant as usize) < published_len
                && !self.is_quarantined(orthant)
            {
                let token = verify(container(orthant), q, verify_counters)
                    .ok_or_else(|| format!("orthant hit {orthant} failed verify"))?;
                counters.orthant_hits += 1;
                return Ok(Some((orthant, token, Hit::Orthant)));
            }
        }
        let probe = Probe::new(Coordinates::of(&q.core), query.word, query.lanes, true);
        let found = bucket
            .index
            .find_controlled(
                Signature::of(&q.core),
                &probe,
                first_id,
                checkpoint,
                &mut Forward {
                    stored: self.stored(),
                    query,
                    candidates: &mut counters.forward_candidates,
                    tests: &mut counters.forward_tests,
                },
            )
            .map_err(str::to_owned)?;
        match found {
            Some(id) => {
                let id = id as u32;
                let token = verify(container(id), q, verify_counters).ok_or_else(|| {
                    format!("index winner {id} failed verify (prefilter/authority disagreement)")
                })?;
                counters.contained_hits += 1;
                Ok(Some((id, token, Hit::Contained)))
            }
            None => {
                counters.misses += 1;
                Ok(None)
            }
        }
    }

    /// Every live ID of `q`'s bucket whose summary `q` contains (ascending),
    /// read-only (P2 reverse set, A4).
    pub fn contained_live(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        counters: &mut LookupCounters,
    ) -> Result<Vec<u32>, String> {
        self.contained_live_bounded(q, query, counters, usize::MAX, || Ok(()))
    }

    /// Bounded read-only retirement preparation. An exceeded allowance or
    /// checkpoint refusal returns no partial set; the caller must discard the
    /// unpublished plan. Limits affect resource admission, never containment.
    pub fn contained_live_bounded(
        &self,
        q: &QueryImage<N>,
        query: &Query<N>,
        counters: &mut LookupCounters,
        limit: usize,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Vec<u32>, String> {
        let Some(&bucket) = self.bucket_of.get(&bucket_key(&q.image)) else {
            return Ok(Vec::new());
        };
        let probe = Probe::new(Coordinates::of(&q.core), query.word, query.lanes, true);
        let ids = self.buckets[bucket as usize]
            .index
            .collect_contained(
                Signature::of(&q.core),
                &probe,
                limit,
                checkpoint,
                &mut Reverse {
                    stored: self.stored(),
                    query,
                    candidates: &mut counters.reverse_candidates,
                    tests: &mut counters.reverse_tests,
                },
            )
            .map_err(str::to_owned)?;
        Ok(ids.into_iter().map(|id| id as u32).collect())
    }

    /// Capacity for `n` new IDs in the arena and summaries (P3 preflight).
    pub fn try_reserve(&mut self, n: usize) -> Result<(), &'static str> {
        if self.rescue_duplicates {
            self.quarantine
                .try_reserve(
                    (self.len() + n)
                        .div_ceil(64)
                        .saturating_sub(self.quarantine.len()),
                )
                .map_err(|_| "quarantine allocation")?;
        }
        self.domains
            .try_reserve(n)
            .map_err(|_| "domain arena allocation")?;
        self.summaries
            .try_reserve(n)
            .map_err(|_| "domain summary allocation")?;
        self.buckets
            .try_reserve(n)
            .map_err(|_| "bucket allocation")?;
        self.bucket_of
            .try_reserve(n)
            .map_err(|_| "bucket allocation")?;
        Ok(())
    }

    /// Append one new ID (P3 step 1, or initial admission): arena, summary,
    /// exact entry, bucket interning. Release checks (SND-7, E3): the digest
    /// is recomputed from the image and must equal `key`, and no equal image
    /// may exist; either failure is an engine inconsistency (C5 in P3).
    /// Allocation cannot fail after `try_reserve`.
    pub fn push(
        &mut self,
        image: CompactDomain<N>,
        summary: CompactSummary<N>,
        key: u64,
    ) -> Result<u32, String> {
        self.push_inner(image, summary, key, false)
    }

    /// Historical equal groups are legitimate after a later quarantine shrinks.
    /// Only the authenticated rescue profile may rebuild such an arena.
    pub fn push_restored(
        &mut self,
        image: CompactDomain<N>,
        summary: CompactSummary<N>,
        key: u64,
    ) -> Result<u32, String> {
        self.push_inner(image, summary, key, true)
    }

    fn push_inner(
        &mut self,
        image: CompactDomain<N>,
        summary: CompactSummary<N>,
        key: u64,
        restoring: bool,
    ) -> Result<u32, String> {
        if image.digest().0 != key {
            return Err("survivor digest differs from its image".into());
        }
        if let Some(existing) = self.exact.get_admissible(key, &image, &self.domains, |id| {
            !self.rescue_duplicates || !restoring && !self.is_quarantined(id)
        }) {
            return Err(format!("image already stored as {existing} (E3)"));
        }
        let id = self.domains.len() as u32;
        self.domains.push(image);
        if self.rescue_duplicates {
            self.quarantine.resize(self.domains.len().div_ceil(64), 0);
        }
        self.summaries.push(summary);
        self.exact.insert(key, id);
        let bucket_key = bucket_key(&image);
        if !self.bucket_of.contains_key(&bucket_key) {
            self.bucket_of.insert(bucket_key, self.buckets.len() as u32);
            self.buckets.push(Bucket {
                index: AggregateIndex::default(),
                orthant: None,
            });
        }
        match image.rank() {
            Some(rank) => {
                self.max_finite_rank = Some(self.max_finite_rank.map_or(rank, |r| r.max(rank)))
            }
            None => self.unbounded_rank_domains += 1,
        }
        self.encountered_rank.observe_domain(&image, Some(&summary));
        Ok(id)
    }

    /// Reserve the index insertion before initial admission publishes an
    /// ID or retires live bits. Failures change capacity only. The caller
    /// has already reserved the store/ledger/exact-index capacities.
    pub fn prepare_initial(
        &mut self,
        image: &CompactDomain<N>,
        query: &Query<N>,
        checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<InitialInsertion<N>, &'static str> {
        let key = bucket_key(image);
        let signature = Signature::of(&query.core);
        let coordinates = Coordinates::of(&query.core);
        let (bucket, new_bucket, insertion) = if let Some(&bucket) = self.bucket_of.get(&key) {
            let insertion = self.buckets[bucket as usize].index.prepare_with(
                signature,
                coordinates,
                checkpoint,
            )?;
            (bucket, None, insertion)
        } else {
            let mut bucket = Bucket {
                index: AggregateIndex::default(),
                orthant: None,
            };
            let insertion = bucket
                .index
                .prepare_with(signature, coordinates, checkpoint)?;
            (self.buckets.len() as u32, Some(bucket), insertion)
        };
        Ok(InitialInsertion {
            id: self.domains.len() as u32,
            bucket,
            key,
            new_bucket,
            insertion,
        })
    }

    /// Consume a prepared initial insertion after its corresponding
    /// `push`. No allocation remains; mismatches are engine failures and
    /// the caller must keep the state poisoned.
    pub fn index_initial(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
        prepared: InitialInsertion<N>,
    ) -> Result<u64, &'static str> {
        if id != prepared.id
            || self.domains.len() != id as usize + 1
            || bucket_key(&self.domains[id as usize]) != prepared.key
            || self.bucket_of.get(&prepared.key) != Some(&prepared.bucket)
        {
            return Err("initial admission prepared index mismatch");
        }
        if let Some(bucket) = prepared.new_bucket {
            if self.buckets.len() != prepared.bucket as usize + 1 {
                return Err("initial admission prepared bucket order mismatch");
            }
            let target = &mut self.buckets[prepared.bucket as usize];
            if target.orthant.is_some() || target.index.storage().live != 0 {
                return Err("initial admission prepared bucket is not empty");
            }
            // `push` interns the bucket at its first-appearance position;
            // replace its empty placeholder with the index we reserved.
            *target = bucket;
        }
        Ok(self.index_prepared(id, query, retire, prepared.bucket, prepared.insertion))
    }

    /// Lookup-index maintenance for one new ID (P3): retire every ID of
    /// `retire` (ascending, all live and contained in `id`, decided in P2),
    /// then index `id` and update the dominant orthant. Returns the number
    /// of index entries removed; the caller checks it against the live bits.
    pub fn index_survivor(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
    ) -> Result<u64, &'static str> {
        let image = self.domains[id as usize];
        let bucket = self.bucket_of[&bucket_key(&image)];
        let signature = Signature::of(&query.core);
        let coordinates = Coordinates::of(&query.core);
        let insertion = self.buckets[bucket as usize]
            .index
            .prepare(signature, coordinates)?;
        Ok(self.index_prepared(id, query, retire, bucket, insertion))
    }

    fn index_prepared(
        &mut self,
        id: u32,
        query: &Query<N>,
        retire: &[u32],
        bucket: u32,
        insertion: Insertion<N>,
    ) -> u64 {
        let image = self.domains[id as usize];
        let bucket = &mut self.buckets[bucket as usize];
        let coordinates = Coordinates::of(&query.core);
        let probe = Probe::new(coordinates, query.word, query.lanes, true);
        let mut visitor = PreparedRetire {
            set: retire,
            retired: 0,
        };
        let removed = bucket.index.retire(&insertion, &probe, &mut visitor) as u64;
        let (word, lanes) = query.image();
        bucket.index.insert(
            insertion,
            Entry {
                id: id as usize,
                coordinates,
                word,
                lanes,
            },
        );
        if image.is_full_orthant()
            && bucket
                .orthant
                .is_none_or(|old| rank_contains(image.rank(), self.domains[old as usize].rank()))
        {
            bucket.orthant = Some(id);
        }
        removed
    }

    pub fn storage_json(&self) -> serde_json::Value {
        #[cfg(test)]
        super::assert_large_finalization_allowed();
        let index = self
            .buckets
            .iter()
            .fold((0usize, 0usize, 0usize), |acc, b| {
                let s = b.index.storage();
                (acc.0 + s.blocks, acc.1 + s.rows, acc.2 + s.live)
            });
        serde_json::json!({
            "domain_bytes": self.domains.capacity() * std::mem::size_of::<CompactDomain<N>>(),
            "summary_bytes": self.summaries.capacity() * std::mem::size_of::<CompactSummary<N>>(),
            "index_block_bytes": index.0, "index_row_bytes": index.1, "live_candidates": index.2,
            "exact_entries": self.exact.len(), "exact_overflow_entries": self.exact.overflow_len(),
            "exact_shards": EXACT_SHARDS, "buckets": self.buckets.len(),
            "scope": "reserved capacities of the S2 canonical store; excludes hash/allocator overhead"})
    }
}
