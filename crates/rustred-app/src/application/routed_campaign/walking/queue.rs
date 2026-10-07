//! Inclusion reuse for one immutable program snapshot, not solved-state reuse.
use super::delegation::{Ledger, SchedulingPolicy};
use rustred::solver::{DomainPowerBounds, DomainPowerError, DomainPowerSummary};
use std::collections::HashMap;
use std::sync::Arc;

mod bits;
pub(super) use bits::SessionCounters;
mod compact;
pub(super) use compact::CompactDomain;
#[cfg(test)]
use compact::{COMPACT_RANGE_ERROR, MAX_COMPACT_COORDINATE};
pub(super) use compact::{CompactSummary, Digest, Query, Stored};
use compact::{ExactIndex, Miss};
mod index;
pub(super) use index::{
    AggregateIndex, Coordinates, Entry, Insertion, Probe, Retire, Signature, Visit,
};
mod checkpoint;
pub(super) use checkpoint::{Metadata as QueueMetadata, SortedBuckets, StoredBuckets};
#[cfg(test)]
pub(super) mod positive_reuse_trace;
mod prepared;
use prepared::PreparedLookup;
pub(super) use prepared::{PREPARED_RETIRE_LIMIT, PreparedAdmission, SpeculativeWork};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub(super) enum Phase {
    Apply,
    Route,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(super) struct Domain<const N: usize> {
    pub phase: Phase,
    #[serde(with = "checkpoint::owner")]
    pub owner: [bool; N],
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    #[serde(with = "checkpoint::powers")]
    pub powers: DomainPowerBounds,
}

impl<const N: usize> Domain<N> {
    /// Full-orthant fixture for the unbounded special case. Production routing
    /// retains the actual supplied box instead of widening it here.
    #[cfg(test)]
    pub fn route_cover(owner: [bool; N], rank: Option<u32>) -> Self {
        Self {
            phase: Phase::Route,
            owner,
            rank,
            powers: DomainPowerBounds::default(),
            lower: vec![0; N],
            upper: vec![None; N],
        }
    }

    /// Sufficient syntactic implication, retained for the explicitly capped
    /// historical scan and to measure additional semantic reuse. The queue
    /// evaluates it as `CompactDomain::contains`; this is the reference form.
    #[cfg(test)]
    pub(super) fn contains(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.owner == other.owner
            && rank_contains(self.rank, other.rank)
            && self.powers.contains(&other.powers)
            && self.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            && self
                .upper
                .iter()
                .zip(&other.upper)
                .all(|(a, b)| a.is_none_or(|a| b.is_some_and(|b| b <= a)))
    }

    pub(super) fn is_full_orthant(&self) -> bool {
        self.lower.len() == N
            && self.upper.len() == N
            && self.powers.is_unconstrained()
            && self.lower.iter().all(|&x| x == 0)
            // Preserve the same box predicate as general containment. The
            // default native lane leaves rank-only orthants unnormalized.
            && self.upper.iter().all(Option::is_none)
    }
}

/// Membership in a rescue bitset (quarantine or abandoned set); false when
/// the bitset is empty or shorter than `id`.
pub(super) fn quarantined_bit(bits: &[u64], id: usize) -> bool {
    compact::quarantined(bits, id)
}

pub(super) fn rank_contains(container: Option<u32>, candidate: Option<u32>) -> bool {
    container.is_none_or(|r| candidate.is_some_and(|s| s <= r))
}

#[derive(serde::Serialize)]
struct OwnerBucket<const N: usize> {
    /// Historical stable full scan, used only in the finite-cap lane.
    ids: Vec<usize>,
    /// Unlimited lane: grouped maximal lookup candidates. Retirement never
    /// removes the exact key, immutable domain or queued work.
    indexed: AggregateIndex<N>,
    /// Largest admitted full-orthant rank; None rank dominates every finite R.
    orthant: Option<usize>,
}

impl<const N: usize> Default for OwnerBucket<N> {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            indexed: AggregateIndex::default(),
            orthant: None,
        }
    }
}

impl<const N: usize> OwnerBucket<N> {
    /// The dominant full orthant that contains a query of this bucket
    /// (`bucket` = its phase/owner code) with `rank` (A1: the stored image's
    /// bucket, full-orthant shape and rank are all checked). The one predicate
    /// of the ordered commit and of the helper's speculative shortcut; restore
    /// also refuses a checkpoint whose orthant ID is not a full orthant.
    fn orthant_hit(
        &self,
        domains: &[CompactDomain<N>],
        quarantine: &[u64],
        (phase, owner): (u8, u32),
        rank: Option<u32>,
    ) -> Option<usize> {
        let id = self.orthant?;
        let stored = &domains[id];
        (!compact::quarantined(quarantine, id)
            && stored.same_bucket(phase, owner)
            && stored.is_full_orthant()
            && rank_contains(stored.rank(), rank))
        .then_some(id)
    }
}

#[cfg(test)]
impl<const N: usize> OwnerBucket<N> {
    fn candidate_ids(&self) -> Vec<usize> {
        let mut ids = self.indexed.ids();
        ids.extend_from_slice(&self.ids);
        ids.sort_unstable();
        ids
    }
}

pub(super) struct Queue<const N: usize> {
    /// Immutable fixed-size image of every admitted domain, indexed by ID.
    /// `domain`/`domain_arc` expand one into the `Domain` transport type.
    pub domains: Vec<CompactDomain<N>>,
    pub(super) delegation: Option<Ledger<(Phase, [bool; N])>>,
    pub next: usize,
    pub deduplicated: usize,
    /// General comparison accounting: actual forward callbacks plus the
    /// conservative aggregate-eligible reverse bound. Coordinate block
    /// rejections can avoid reverse callbacks without reducing that charge.
    /// Hash equality and indexed rank/coordinate tests are not charged.
    pub containment_checks: usize,
    /// Conservative reverse-maintenance comparison charges, included in
    /// containment_checks. Zero in the unchanged finite-comparison-cap lane.
    pub containment_maintenance_checks: usize,
    /// Cumulative IDs removed only from the containment candidate index.
    /// Every domain/exact key remains retained. The opt-in responsibility
    /// ledger may delegate an untouched obligation to its containing successor.
    pub containment_retired_candidates: usize,
    /// Successfully constructed cached geometry, including rejected/reused
    /// requests. Exact-key hits do not construct another summary.
    pub containment_summary_builds: usize,
    /// Successful forward/reverse implications missed by the old raw bounds.
    pub containment_semantic_hits: usize,
    pub containment_semantic_retirements: usize,
    pub exact_hits: usize,
    pub orthant_hits: usize,
    pub max_finite_rank: Option<u32>,
    pub unbounded_rank_domains: usize,
    pub encountered_rank: super::rank_telemetry::RankCensus,
    /// Digest-keyed exact index; every hit is confirmed on the stored domain.
    exact: ExactIndex<N>,
    by_owner: HashMap<(Phase, [bool; N]), OwnerBucket<N>>,
    /// Indexed by ID like the domains: the immutable compact native summary
    /// of every admitted ID in the unlimited lane (none in the finite-cap
    /// lane). Never released, so any reader may read any admitted ID. The
    /// kernel's filter words and lanes live inline in the index blocks and
    /// are rebuilt from these summaries on restore, never persisted.
    summaries: Vec<CompactSummary<N>>,
    /// Coordinator-side filter telemetry for this process session only.
    pub(super) session: SessionCounters,
    prefilter: bits::Prefilter,
    max_domains: usize,
    max_checks: Option<usize>,
    /// Separates immutable lookup preparations from unrelated queue instances.
    identity: Arc<()>,
    /// Resume-time rescue quarantine (`rescue.rs`): one bit per admitted ID
    /// whose cone reaches a frontier. Empty (no allocation, no effect) unless
    /// the walk carries an input amendment; never persisted (recomputed from
    /// the dependency monitor at every resume of an amended walk).
    quarantine: Vec<u64>,
    /// Rescue: unpublished, non-delegated obligations no live input root
    /// reaches (their cones cannot certify any query). Published without a
    /// native inspection as unsealed `rescue_abandoned` records. Empty unless
    /// amended; never persisted (recomputed at every amended resume). Shared
    /// with the inspection workers, which read it at every dispatch; the
    /// coordinator extends it when a dead obligation still being inspected
    /// (a restored partial prefix) admits a new domain (`mark_dead`).
    abandoned: Option<Arc<std::sync::RwLock<Vec<u64>>>>,
    /// Test instrumentation preference only, deliberately not persisted.
    #[cfg(test)]
    index_work_counters_enabled: bool,
}

impl<const N: usize> Queue<N> {
    /// The admitted domain `id` in its transport form (allocates two vectors).
    pub fn domain(&self, id: usize) -> Domain<N> {
        self.domains[id].expand()
    }

    /// Shared transport handle for an inspection slot or a physical split.
    pub fn domain_arc(&self, id: usize) -> Arc<Domain<N>> {
        Arc::new(self.domain(id))
    }

    /// Transport handles of the first `n` admissions: the bounded initial
    /// prefix read by the initial-orthant and initial-overlap indexes, or the
    /// whole queue for the owner-batched handoff.
    pub fn expand_prefix(&self, n: usize) -> Vec<Arc<Domain<N>>> {
        self.domains[..n]
            .iter()
            .map(|domain| Arc::new(domain.expand()))
            .collect()
    }

    /// Reserved bytes of the compact queue state: per-ID images, exact index,
    /// per-ID summaries and the candidate index. O(owner buckets): the index
    /// keeps running totals, because every coordinator heartbeat calls this
    /// twice. The ledger and closure are accounted elsewhere.
    pub fn storage_json(&self) -> serde_json::Value {
        let domains = self.domains.capacity() * std::mem::size_of::<CompactDomain<N>>();
        let exact = self.exact.capacity_bytes();
        let summaries = self.summaries.capacity() * std::mem::size_of::<CompactSummary<N>>();
        let index = self
            .by_owner
            .values()
            .map(|bucket| bucket.indexed.storage())
            .fold(index::IndexBytes::default(), |a, b| index::IndexBytes {
                blocks: a.blocks + b.blocks,
                rows: a.rows + b.rows,
                live: a.live + b.live,
                lossy: a.lossy + b.lossy,
            });
        let per_id = domains + exact + summaries;
        let total = per_id + index.blocks + index.rows;
        serde_json::json!({"domain_bytes":domains,"exact_index_bytes":exact,
            "summary_bytes":summaries,"index_block_bytes":index.blocks,
            "index_row_bytes":index.rows,"total_bytes":total,
            "total_excluding_index_bytes":per_id,
            "admitted_domains":self.domains.len(),"live_candidates":index.live,
            "lossy_lane_candidates":index.lossy,
            "bytes_per_admitted_domain":(!self.domains.is_empty())
                .then(|| total as f64 / self.domains.len() as f64),
            // Pre-kernel binaries (4a17f9c7, 7eed68fc) reported a total
            // without the candidate index, over a live-only summary slab and
            // 8-B filter words per ID; total_excluding_index_bytes is the
            // nearest analogue. Memory gates compare process RSS.
            "scope":"compact queue state incl. SoA candidate index; reserved capacities; excl. allocator/hash overhead, ledger, closure; pre-kernel total_bytes ~ total_excluding_index_bytes"})
    }

    /// The kernel probe of `query`.
    fn probe<'q>(&self, query: &'q Query<N>) -> Probe<'q, N> {
        Probe::new(
            Coordinates::of(&query.core),
            query.word,
            query.lanes,
            self.prefilter.enabled(),
        )
    }

    fn stored(&self) -> Stored<'_, N> {
        Stored {
            domains: &self.domains,
            summaries: &self.summaries,
            quarantine: &self.quarantine,
        }
    }

    /// Install the rescue quarantine (see the field). Lookups (exact index,
    /// dominant orthant, candidate index, helper preparations) then never
    /// return a quarantined ID; admission of an exact duplicate of one
    /// creates a fresh ID. Refuses a bitset of the wrong size. The quarantine
    /// is not monotone (a dead obligation published cleanly later, e.g. as
    /// an alias of a live representative, is live again), so an exact-
    /// duplicate group may hold several live members: every one is a valid
    /// exact container, and lookups return the oldest live one.
    pub fn install_quarantine(&mut self, bits: Vec<u64>) -> Result<usize, String> {
        if bits.len() != self.domains.len().div_ceil(64)
            || bits.last().is_some_and(|&w| {
                self.domains.len() % 64 != 0 && w >> (self.domains.len() % 64) != 0
            })
        {
            return Err("rescue quarantine does not match the admitted domains".into());
        }
        let count = bits.iter().map(|w| w.count_ones() as usize).sum();
        self.quarantine = if count == 0 { Vec::new() } else { bits };
        // A dominant orthant that is quarantined is skipped by `orthant_hit`;
        // a later live full orthant replaces it (see `admit_with_lookup`).
        Ok(count)
    }

    pub fn is_quarantined(&self, id: usize) -> bool {
        compact::quarantined(&self.quarantine, id)
    }

    /// IDs admitted after the install are beyond the bitset: never quarantined.
    pub fn quarantine_active(&self) -> bool {
        !self.quarantine.is_empty()
    }

    /// Install the rescue's abandoned set (see the field); every member
    /// must already be quarantined, so no later lookup resolves into it.
    pub fn set_abandoned(&mut self, bits: Vec<u64>) -> Result<usize, String> {
        if bits.len() > self.domains.len().div_ceil(64)
            || bits
                .iter()
                .enumerate()
                .any(|(word, &b)| b & !self.quarantine.get(word).copied().unwrap_or(0) != 0)
        {
            return Err("rescue abandoned set outside the quarantine".into());
        }
        let count = bits.iter().map(|w| w.count_ones() as usize).sum();
        self.abandoned = self
            .quarantine_active()
            .then(|| Arc::new(std::sync::RwLock::new(bits)));
        Ok(count)
    }

    /// The shared abandoned set (None for every unamended walk).
    pub fn abandoned_handle(&self) -> Option<Arc<std::sync::RwLock<Vec<u64>>>> {
        self.abandoned.clone()
    }

    /// A domain newly admitted by a quarantined (dead) source is itself dead:
    /// quarantine it and abandon it (published without inspection).
    pub fn mark_dead(&mut self, id: usize) {
        let word = id / 64;
        if self.quarantine.len() <= word {
            self.quarantine.resize(word + 1, 0);
        }
        self.quarantine[word] |= 1 << (id % 64);
        if let Some(set) = &self.abandoned {
            let mut bits = set.write().unwrap_or_else(|e| e.into_inner());
            if bits.len() <= word {
                bits.resize(word + 1, 0);
            }
            bits[word] |= 1 << (id % 64);
        }
    }

    pub fn containment_limit(&self) -> Option<usize> {
        self.max_checks
    }

    /// Current indexed candidate count, without scanning owner buckets.
    pub fn containment_candidate_count(&self) -> usize {
        self.domains.len() - self.containment_retired_candidates
    }

    pub fn containment_index_policy(&self) -> &'static str {
        if self.max_checks.is_none() {
            "maximal_candidates_semantic_unlimited"
        } else {
            "historical_candidates_finite_cap"
        }
    }

    /// Checked aggregate accounting for a producer's earlier ordered Admit.
    /// Exact/orthant counters are deliberately unchanged: the bypassed lookup
    /// might instead have needed arbitrary-box comparisons.
    pub fn count_known_reuse(&mut self, count: usize) -> Result<(), &'static str> {
        self.deduplicated = self
            .deduplicated
            .checked_add(count)
            .ok_or("reuse counter overflow")?;
        Ok(())
    }
    pub fn new(max_domains: usize, max_checks: Option<usize>) -> Self {
        Self {
            domains: Vec::new(),
            delegation: None,
            next: 0,
            deduplicated: 0,
            containment_checks: 0,
            containment_maintenance_checks: 0,
            containment_retired_candidates: 0,
            containment_summary_builds: 0,
            containment_semantic_hits: 0,
            containment_semantic_retirements: 0,
            exact_hits: 0,
            orthant_hits: 0,
            max_finite_rank: None,
            unbounded_rank_domains: 0,
            encountered_rank: Default::default(),
            exact: ExactIndex::new(),
            by_owner: HashMap::new(),
            summaries: Vec::new(),
            session: SessionCounters::default(),
            prefilter: bits::Prefilter::new(),
            max_domains,
            max_checks,
            identity: Arc::new(()),
            quarantine: Vec::new(),
            abandoned: None,
            #[cfg(test)]
            index_work_counters_enabled: true,
        }
    }

    /// Test seam only: prove that admission results and persisted counters do
    /// not depend on the bit-signature tier. Not a policy and not persisted.
    #[cfg(test)]
    pub fn disable_bit_prefilter(&mut self) {
        self.prefilter.disable();
    }

    /// The kernel word of every admitted ID of the unlimited lane, rebuilt
    /// from its immutable summary (none in the finite-cap lane).
    #[cfg(test)]
    pub(super) fn bit_words(&self) -> Vec<u64> {
        self.domains
            .iter()
            .zip(&self.summaries)
            .map(|(domain, summary)| compact::stored_image(domain, summary).unwrap().0)
            .collect()
    }

    /// Whether `id` is a live candidate of the unlimited lane's index.
    #[cfg(test)]
    pub(super) fn is_indexed(&self, id: usize) -> bool {
        let domain = &self.domains[id];
        self.by_owner
            .get(&(domain.phase(), domain.owner()))
            .is_some_and(|bucket| bucket.indexed.is_live(self.stored().signature(id), id))
    }

    pub fn with_policy(
        max_domains: usize,
        max_checks: Option<usize>,
        policy: SchedulingPolicy,
    ) -> Result<Self, String> {
        policy
            .validate(max_checks)
            .map_err(|error| error.to_string())?;
        let mut queue = Self::new(max_domains, max_checks);
        if let SchedulingPolicy::TransferUnreserved { lookahead } = policy {
            queue.delegation =
                Some(Ledger::new(lookahead, max_domains).map_err(|error| error.to_string())?);
        }
        Ok(queue)
    }

    /// A pending containing domain can suppress another scheduling request, but
    /// every admitted responsibility still has to finish before exhaustion.
    /// InspectAll requires each native inspection; optional delegation requires
    /// the explicitly tracked containing representative instead.
    /// The queue is never shared between different snapshots or rank policies.
    /// Exact/full-orthant index proofs do not spend general containment checks,
    /// so they can still succeed at a finite comparison cap or counter maximum.
    /// None means no policy cap, but counter overflow remains an explicit error.
    /// A dominant orthant or maximal candidate can return a different valid
    /// containing ID than the historical first-match scan. Exact IDs and all
    /// already admitted IDs/FIFO obligations remain unchanged. Stronger exact
    /// inclusion may avoid admissions that the former raw predicate retained.
    /// Finite-cap mode deliberately keeps its existing raw full-scan policy
    /// and performs no reverse maintenance or summary construction work.
    pub fn admit(&mut self, domain: Domain<N>) -> Result<(usize, bool), &'static str> {
        self.admit_with_lookup(domain, None, None)
    }

    /// Narrow a destination queue's allowance to its remaining share of a
    /// global budget. Reuse is still attempted at the domain cap. Never change
    /// the finite/unlimited lane: that would invalidate its existing index.
    pub fn admit_with_budget(
        &mut self,
        domain: Domain<N>,
        domain_limit: usize,
        comparison_limit: Option<usize>,
    ) -> Result<(usize, bool), &'static str> {
        if domain_limit < self.domains.len()
            || domain_limit > self.max_domains
            || comparison_limit.is_some() != self.max_checks.is_some()
            || matches!((comparison_limit, self.max_checks), (Some(a), Some(b)) if a > b)
            || comparison_limit.is_some_and(|limit| limit < self.containment_checks)
        {
            return Err("invalid narrowed admission budget");
        }
        let saved_domains = std::mem::replace(&mut self.max_domains, domain_limit);
        let saved_checks = std::mem::replace(&mut self.max_checks, comparison_limit);
        let result = self.admit(domain);
        self.max_domains = saved_domains;
        self.max_checks = saved_checks;
        result
    }

    /// `key`: the compact image and digest of `domain` when a helper of this
    /// same queue already computed them (`PreparedAdmission`).
    fn admit_with_lookup(
        &mut self,
        domain: Domain<N>,
        mut prepared: Option<PreparedLookup<N>>,
        key: Option<(CompactDomain<N>, Digest)>,
    ) -> Result<(usize, bool), &'static str> {
        debug_assert_eq!(domain.lower.len(), N);
        debug_assert_eq!(domain.upper.len(), N);
        #[cfg(test)]
        let observation = positive_reuse_trace::begin(self, &domain);
        let (compact, key) = match key {
            Some(prepared) => prepared,
            None => {
                // The compact queue's only refusal: a finite coordinate above 65534.
                let compact = CompactDomain::try_from_domain(&domain)?;
                (compact, self.exact.key(&compact))
            }
        };
        debug_assert_eq!(compact.expand(), domain);
        // Digest lookup confirmed on the stored domain: a digest collision can
        // cost a comparison, never a wrong hit. Nothing is allocated here.
        let exact_miss = match self
            .exact
            .get_live(key, &compact, &self.domains, &self.quarantine)
        {
            Ok(id) => {
                self.exact_hits += 1;
                self.deduplicated += 1;
                return Ok((id, false));
            }
            Err(miss) => miss,
        };
        let query = if self.max_checks.is_none() {
            let builds = self
                .containment_summary_builds
                .checked_add(1)
                .ok_or("domain summary counter overflow")?;
            let query = if let Some(prepared) = &prepared {
                prepared.query.clone()
            } else {
                Query::new(
                    DomainPowerSummary::try_new(
                        domain.owner,
                        &domain.lower,
                        &domain.upper,
                        domain.rank,
                        domain.powers,
                    )
                    .map_err(summary_error)?,
                    domain.phase,
                )
            };
            self.containment_summary_builds = builds;
            Some(query)
        } else {
            None
        };
        let filter = self.prefilter.enabled();
        let bucket_key = (domain.phase, domain.owner);
        let (phase_code, owner_code) = compact.bucket_code();
        // Helper-prepared reverse retirement set with its snapshot watermark;
        // only a revalidated prepared miss can supply one.
        let mut prepared_retire: Option<(Vec<usize>, usize)> = None;
        let mut trivial_retire = false;
        if let Some(bucket) = self.by_owner.get(&bucket_key) {
            // A1: the shortcut is a positive only after an explicit check of
            // the stored image's bucket, full orthant and rank.
            if let Some(id) = bucket.orthant_hit(
                &self.domains,
                &self.quarantine,
                (phase_code, owner_code),
                domain.rank,
            ) {
                self.orthant_hits += 1;
                self.deduplicated += 1;
                return Ok((id, false));
            }
            let found = if let Some(query) = &query {
                let stored = Stored {
                    domains: &self.domains,
                    summaries: &self.summaries,
                    quarantine: &self.quarantine,
                };
                if let Some(revalidated) = prepared.as_mut().and_then(|lookup| {
                    lookup.revalidate(
                        &bucket.indexed,
                        stored,
                        filter,
                        self.containment_checks,
                        &mut self.session,
                    )
                }) {
                    self.containment_checks += revalidated.checks; // checked by revalidate
                    if revalidated.found.is_none() {
                        prepared_retire =
                            revalidated.retire.map(|set| (set, revalidated.first_new));
                        trivial_retire = revalidated.trivial;
                    }
                    revalidated.found
                } else {
                    let probe = Probe::new(
                        Coordinates::of(&query.core),
                        query.word,
                        query.lanes,
                        filter,
                    );
                    let started = std::time::Instant::now();
                    let found = bucket.indexed.find_from(
                        Signature::of(&query.core),
                        &probe,
                        0,
                        &mut Charged {
                            checks: &mut self.containment_checks,
                            session: &mut self.session,
                            stored,
                            query,
                        },
                    )?;
                    self.session.forward_scan(started);
                    found
                }
            } else {
                let mut found = None;
                for &id in &bucket.ids {
                    if self
                        .max_checks
                        .is_some_and(|limit| self.containment_checks >= limit)
                    {
                        return Err("domain containment check allowance");
                    }
                    self.containment_checks = self
                        .containment_checks
                        .checked_add(1)
                        .ok_or("domain containment counter overflow")?;
                    if self.domains[id].contains(&compact)
                        && !compact::quarantined(&self.quarantine, id)
                    {
                        found = Some(id);
                        break;
                    }
                }
                found
            };
            if let Some(id) = found {
                let semantic = query.is_some() && !self.domains[id].contains(&compact);
                if semantic {
                    self.containment_semantic_hits = self
                        .containment_semantic_hits
                        .checked_add(1)
                        .ok_or("semantic containment hit counter overflow")?;
                }
                #[cfg(test)]
                positive_reuse_trace::positive(self, &domain, id, semantic, observation);
                self.deduplicated += 1;
                return Ok((id, false));
            }
        }
        if self.domains.len() == self.max_domains {
            return Err("scheduled domain allowance");
        }
        let id = self.domains.len();
        if query.is_some() && id >= u32::MAX as usize {
            // Index blocks store u32 IDs (like the summary and ledger slots).
            return Err("candidate index ID range");
        }
        if let Some(ledger) = &mut self.delegation {
            ledger
                .reserve_admission(id)
                .map_err(|_| "delegation ledger admission allocation")?;
        }
        self.domains
            .try_reserve(1)
            .map_err(|_| "domain allocation")?;
        self.exact.try_reserve(key, exact_miss)?;
        if query.is_some() {
            self.summaries
                .try_reserve(1)
                .map_err(|_| "domain summary allocation")?;
        }
        // Reserve every fallible collection slot before publishing the domain,
        // either index, or rank telemetry. Failed reserves may change capacity,
        // never logical admission state. Occasional native HashMap rehash is
        // O(admitted domains); hash iteration never determines queue semantics.
        let signature = query.as_ref().map(|query| Signature::of(&query.core));
        let coordinates = query
            .as_ref()
            .and_then(|query| Coordinates::of(&query.core));
        let (new_bucket, insertion) = if let Some(bucket) = self.by_owner.get_mut(&bucket_key) {
            let insertion = if let Some(signature) = signature {
                Some(bucket.indexed.prepare(signature, coordinates)?)
            } else {
                bucket
                    .ids
                    .try_reserve(1)
                    .map_err(|_| "owner domain index allocation")?;
                None
            };
            (None, insertion)
        } else {
            self.by_owner
                .try_reserve(1)
                .map_err(|_| "owner domain index allocation")?;
            let mut bucket = OwnerBucket::default();
            #[cfg(test)]
            bucket
                .indexed
                .set_work_counters_enabled(self.index_work_counters_enabled);
            let insertion = if let Some(signature) = signature {
                Some(bucket.indexed.prepare(signature, coordinates)?)
            } else {
                bucket
                    .ids
                    .try_reserve(1)
                    .map_err(|_| "owner domain index allocation")?;
                None
            };
            (Some(bucket), insertion)
        };
        let full_orthant = compact.is_full_orthant();
        // Preflight all reverse comparisons before changing the candidate
        // index or publishing this admission. A failed allocation above or a
        // counter overflow here can only alter reserved capacity/work counters,
        // never retire an obligation's only indexed representative.
        let maintenance = if let Some(signature) = signature {
            self.by_owner
                .get(&bucket_key)
                .map_or(Ok(0), |bucket| bucket.indexed.maintenance_len(signature))?
        } else {
            0
        };
        let total_checks = self
            .containment_checks
            .checked_add(maintenance)
            .ok_or("domain containment counter overflow")?;
        let maintenance_checks = self
            .containment_maintenance_checks
            .checked_add(maintenance)
            .ok_or("domain containment maintenance counter overflow")?;
        // At most every examined candidate can be retired. Preflight that
        // upper bound so the in-place retain needs no fallible post-mutation
        // accounting or a second geometry scan just to count removals.
        self.containment_retired_candidates
            .checked_add(maintenance)
            .ok_or("domain containment retired-candidate counter overflow")?;
        self.containment_semantic_retirements
            .checked_add(maintenance)
            .ok_or("semantic containment retirement counter overflow")?;
        if let Some(ledger) = &mut self.delegation {
            // Last fallible ledger operation before the infallible queue
            // retirement/publication transaction. No observer runs mid-commit.
            ledger
                .admit_reserved(id, bucket_key)
                .map_err(|_| "delegation ledger admission invariant")?;
        }
        let fresh_bucket = new_bucket.is_some();
        if let Some(bucket) = new_bucket {
            self.by_owner.insert(bucket_key, bucket);
        }
        let bucket = self
            .by_owner
            .get_mut(&bucket_key)
            .expect("reserved owner bucket");
        let mut extra_retired = 0;
        let retired = if let Some(insertion) = insertion.as_ref() {
            // All retained candidates and the new domain belong to this same
            // immutable phase/owner snapshot. Transitivity preserves a retained
            // containing representative for every retired candidate. Keep the
            // old domains/exact entries. InspectAll retains every FIFO job;
            // optional transfer changes only untouched, unreserved responsibility.
            let query = query.as_ref().expect("unlimited lane query");
            // A brand-new bucket has nothing to retire on either path.
            if prepared.is_some() && !fresh_bucket {
                if prepared_retire.is_some() && trivial_retire {
                    // The bucket appeared after the snapshot (an earlier
                    // commit of the batch): the empty set decided nothing.
                    self.session.prepared_retirements_trivial =
                        self.session.prepared_retirements_trivial.saturating_add(1);
                } else if prepared_retire.is_some() {
                    self.session.prepared_retirements_applied =
                        self.session.prepared_retirements_applied.saturating_add(1);
                } else {
                    self.session.prepared_retire_fallbacks =
                        self.session.prepared_retire_fallbacks.saturating_add(1);
                }
            }
            let probe = Probe::new(coordinates, query.word, query.lanes, filter);
            let domains = &self.domains;
            let delegation = &mut self.delegation;
            let extra_retired = &mut extra_retired;
            // Apply-time effects of one retirement, identical on both paths.
            let on_retire = |old: usize| {
                if !compact.contains(&domains[old]) {
                    *extra_retired += 1; // preflighted by the maintenance bound
                }
                if let Some(ledger) = delegation.as_mut() {
                    // Same immutable phase/owner bucket; exact native inclusion
                    // is the authority. Protected work remains a native obligation.
                    let _ = ledger.transfer_retired(old, id);
                }
            };
            // Exact inclusion of an old candidate, charged as a reverse callback.
            let mut reverse = Reverse {
                session: &mut self.session,
                stored: Stored {
                    domains,
                    summaries: &self.summaries,
                    quarantine: &self.quarantine,
                },
                query,
            };
            let started = std::time::Instant::now();
            let removed = match prepared_retire.take() {
                Some((set, first_new)) => bucket.indexed.retire_prepared(
                    insertion,
                    &probe,
                    &set,
                    first_new,
                    &mut reverse,
                    on_retire,
                ),
                None => bucket.indexed.retire(
                    insertion,
                    &probe,
                    &mut WithEffects {
                        inner: reverse,
                        on_retire,
                    },
                ),
            };
            self.session.reverse_scan(started);
            removed
        } else {
            0
        };
        self.containment_checks = total_checks;
        self.containment_maintenance_checks = maintenance_checks;
        self.containment_retired_candidates += retired;
        self.containment_semantic_retirements += extra_retired;
        if let Some(insertion) = insertion {
            let (word, lanes) = query.as_ref().expect("unlimited lane query").image();
            bucket.indexed.insert(
                insertion,
                Entry {
                    id,
                    coordinates,
                    word,
                    lanes,
                },
            );
        } else {
            bucket.ids.push(id);
        }
        if full_orthant
            && bucket.orthant.is_none_or(|old| {
                rank_contains(domain.rank, self.domains[old].rank())
                    || compact::quarantined(&self.quarantine, old)
            })
        {
            bucket.orthant = Some(id);
        }
        if let Some(rank) = domain.rank {
            self.max_finite_rank = Some(self.max_finite_rank.map_or(rank, |old| old.max(rank)));
        } else {
            self.unbounded_rank_domains += 1;
        }
        self.encountered_rank
            .observe_domain(&compact, query.as_ref().map(|query| &query.compact));
        self.exact.insert(key, id);
        self.domains.push(compact);
        if let Some(query) = query {
            self.summaries.push(query.compact);
        }
        Ok((id, true))
    }
}

/// Ordered-commit forward visitor: charges every logical candidate, exactly
/// as the per-ID callback did (prefilter rejections included).
struct Charged<'a, const N: usize> {
    checks: &'a mut usize,
    session: &'a mut SessionCounters,
    stored: Stored<'a, N>,
    query: &'a Query<N>,
}

impl<const N: usize> Visit for Charged<'_, N> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        *self.checks = self
            .checks
            .checked_add(run.len())
            .ok_or("domain containment counter overflow")?;
        self.session
            .forward_run(run.len(), word.count_ones() as usize);
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        *self.checks = self
            .checks
            .checked_add(1)
            .ok_or("domain containment counter overflow")?;
        self.session.forward_test();
        Ok(self.stored.contains(id, self.query))
    }
}

/// Ordered-commit reverse visitor: the exact inclusion of an old candidate,
/// with the session's reverse-callback telemetry.
struct Reverse<'a, const N: usize> {
    session: &'a mut SessionCounters,
    stored: Stored<'a, N>,
    query: &'a Query<N>,
}

impl<const N: usize> Retire for Reverse<'_, N> {
    fn rejected(&mut self, run: &[u32], word: u32) {
        self.session
            .reverse_run(run.len(), word.count_ones() as usize);
    }
    fn test(&mut self, id: usize) -> bool {
        self.session.reverse_test();
        self.stored.contained_by(id, self.query)
    }
    fn decided(&mut self, count: usize) {
        self.session.reverse_decided(count);
    }
}

/// A reverse visitor plus the apply-time effects of each retirement.
struct WithEffects<V, F> {
    inner: V,
    on_retire: F,
}

impl<V: Retire, F: FnMut(usize)> Retire for WithEffects<V, F> {
    fn rejected(&mut self, run: &[u32], word: u32) {
        self.inner.rejected(run, word);
    }
    fn test(&mut self, id: usize) -> bool {
        let retire = self.inner.test(id);
        if retire {
            (self.on_retire)(id);
        }
        retire
    }
    fn decided(&mut self, count: usize) {
        self.inner.decided(count);
    }
}

fn summary_error(error: DomainPowerError) -> &'static str {
    match error {
        DomainPowerError::InvalidArity => "invalid domain summary arity",
        DomainPowerError::InvertedCoordinate { .. } => "inverted domain summary coordinate bounds",
        DomainPowerError::InvertedDifferenceBounds => "inverted domain summary difference bounds",
        DomainPowerError::ArithmeticOverflow(context) | DomainPowerError::OutOfRange(context) => {
            context
        }
    }
}

#[cfg(test)]
mod tests;
