//! Streaming queue image. Index membership/order is preserved, not re-admitted.
//! CP5 persists the parts separately: scalar metadata (JSON), immutable domain
//! records (append-only segments), owner buckets sorted by (phase, owner) so
//! their bytes are deterministic, and the responsibility ledger.
use super::super::checkpoint::restore::Phases;
use super::super::delegation::{LedgerRef, StoredLedger};
use super::*;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(super) mod owner {
    use super::*;
    pub fn serialize<S: Serializer, const N: usize>(
        v: &[bool; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        v.as_slice().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[bool; N], D::Error> {
        crate::application::routed_campaign::storage::restore_array(
            &Vec::<bool>::deserialize(d)?,
            false,
        )
        .ok_or_else(|| serde::de::Error::custom("checkpoint owner arity"))
    }
}
pub(super) mod powers {
    use super::*;
    pub fn serialize<S: Serializer>(v: &DomainPowerBounds, s: S) -> Result<S::Ok, S::Error> {
        (
            v.max_positive_power,
            v.min_power_difference,
            v.max_power_difference,
        )
            .serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DomainPowerBounds, D::Error> {
        let (max_positive_power, min_power_difference, max_power_difference) =
            Deserialize::deserialize(d)?;
        Ok(DomainPowerBounds {
            max_positive_power,
            min_power_difference,
            max_power_difference,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Metadata {
    next: usize,
    deduplicated: usize,
    containment_checks: usize,
    containment_maintenance_checks: usize,
    containment_retired_candidates: usize,
    containment_summary_builds: usize,
    containment_semantic_hits: usize,
    containment_semantic_retirements: usize,
    exact_hits: usize,
    orthant_hits: usize,
    max_finite_rank: Option<u32>,
    unbounded_rank_domains: usize,
    max_domains: usize,
    max_checks: Option<usize>,
}
/// Serialized as the transport `Domain` records, one expansion at a time.
struct Domains<'a, const N: usize>(&'a [CompactDomain<N>]);
impl<const N: usize> Serialize for Domains<'_, N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for domain in self.0 {
            seq.serialize_element(&domain.expand())?;
        }
        seq.end()
    }
}
/// Owner buckets in (phase, owner) order: identical queue state yields
/// identical bytes regardless of hash-map iteration order.
pub(in super::super) struct SortedBuckets<'a, const N: usize>(
    Vec<(&'a (Phase, [bool; N]), &'a OwnerBucket<N>)>,
);
impl<const N: usize> SortedBuckets<'_, N> {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
impl<const N: usize> Serialize for SortedBuckets<'_, N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for ((phase, owner), bucket) in &self.0 {
            seq.serialize_element(&(phase, owner.as_slice(), bucket))?;
        }
        seq.end()
    }
}
/// The CP5 image of one owner bucket (the live `OwnerBucket` serializes to
/// exactly this shape).
#[derive(Serialize, Deserialize)]
#[serde(rename = "OwnerBucket")]
struct StoredOwnerBucket {
    ids: Vec<usize>,
    indexed: index::StoredIndex,
    orthant: Option<usize>,
}
/// Decoded bucket image awaiting validation against the restored domains.
#[derive(Serialize, Deserialize)]
pub(in super::super) struct StoredBuckets(Vec<(Phase, Vec<bool>, StoredOwnerBucket)>);
impl StoredBuckets {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
impl Metadata {
    /// The saved contiguous publication cursor (the Ordered publisher).
    pub(in super::super) fn next(&self) -> usize {
        self.next
    }
}
impl<const N: usize> Queue<N> {
    pub(in super::super) fn checkpoint_metadata(&self) -> Metadata {
        Metadata {
            next: self.next,
            deduplicated: self.deduplicated,
            containment_checks: self.containment_checks,
            containment_maintenance_checks: self.containment_maintenance_checks,
            containment_retired_candidates: self.containment_retired_candidates,
            containment_summary_builds: self.containment_summary_builds,
            containment_semantic_hits: self.containment_semantic_hits,
            containment_semantic_retirements: self.containment_semantic_retirements,
            exact_hits: self.exact_hits,
            orthant_hits: self.orthant_hits,
            max_finite_rank: self.max_finite_rank,
            unbounded_rank_domains: self.unbounded_rank_domains,
            max_domains: self.max_domains,
            max_checks: self.max_checks,
        }
    }
    pub(in super::super) fn checkpoint_buckets(&self) -> SortedBuckets<'_, N> {
        let mut buckets: Vec<_> = self.by_owner.iter().collect();
        buckets.sort_unstable_by_key(|(key, _)| *key);
        SortedBuckets(buckets)
    }
    pub(in super::super) fn checkpoint_ledger(&self) -> Option<LedgerRef<'_, (Phase, [bool; N])>> {
        self.delegation.as_ref().map(LedgerRef)
    }
    /// Validate and rebuild lookup structures; never re-admit or reorder.
    /// Domains arrive already range-checked (`CompactDomain::restore`).
    #[cfg(test)]
    pub(in super::super) fn restore_from_parts(
        m: Metadata,
        domains: Vec<CompactDomain<N>>,
        buckets: StoredBuckets,
        ledger: Option<StoredLedger>,
        phases: &mut Phases,
    ) -> Result<Self, String> {
        Self::restore_with_index(
            m,
            domains,
            buckets,
            ledger,
            ExactIndex::new(),
            false,
            phases,
        )
    }

    /// `restore_from_parts` of an amended walk (`rescue.rs`): exact duplicates
    /// are admitted (the rescue re-admits a domain whose earlier ID is
    /// quarantined); `install_quarantine` then checks every duplicate group.
    pub(in super::super) fn restore_from_parts_amended(
        m: Metadata,
        domains: Vec<CompactDomain<N>>,
        buckets: StoredBuckets,
        ledger: Option<StoredLedger>,
        amended: bool,
        phases: &mut Phases,
    ) -> Result<Self, String> {
        Self::restore_with_index(
            m,
            domains,
            buckets,
            ledger,
            ExactIndex::new(),
            amended,
            phases,
        )
    }

    /// `restore_from_parts` into an empty exact index, whose key function a
    /// test may have replaced to restore under forced digest collisions.
    fn restore_with_index(
        m: Metadata,
        domains: Vec<CompactDomain<N>>,
        buckets: StoredBuckets,
        ledger: Option<StoredLedger>,
        exact: ExactIndex<N>,
        duplicates: bool,
        phases: &mut Phases,
    ) -> Result<Self, String> {
        if m.next > domains.len()
            || domains.len() > m.max_domains
            || m.containment_retired_candidates > domains.len()
        {
            return Err("invalid checkpoint queue counters".into());
        }
        let started = std::time::Instant::now();
        let mut q = Queue::new(m.max_domains, m.max_checks);
        q.exact = exact;
        q.exact
            .try_reserve_total(domains.len())
            .map_err(|_| "checkpoint exact index allocation")?;
        q.domains = domains;
        for (id, domain) in q.domains.iter().enumerate() {
            // The capped lane has no retained summaries; rebuild its tiny
            // diagnostic extent in the existing restore pass, not per tick.
            if m.max_checks.is_some() {
                q.encountered_rank.observe_domain(domain, None);
            }
            let key = q.exact.key(domain);
            let miss = match q.exact.get(key, domain, &q.domains) {
                Err(miss) => miss,
                Ok(_) if duplicates => Miss::duplicate(),
                Ok(_) => return Err("duplicate checkpoint exact domain".into()),
            };
            q.exact
                .try_reserve(key, miss)
                .map_err(|_| "checkpoint exact index allocation")?;
            q.exact.insert(key, id);
        }
        phases.since("domains_exact_index", started);
        let started = std::time::Instant::now();
        let mut stored = Vec::new();
        stored
            .try_reserve_exact(buckets.0.len())
            .map_err(|_| "checkpoint owner bucket allocation")?;
        let mut keys = std::collections::HashSet::new();
        for (phase, owner, bucket) in buckets.0 {
            let owner: [bool; N] = owner.try_into().map_err(|_| "checkpoint bucket arity")?;
            if bucket.ids.iter().chain(bucket.orthant.iter()).any(|&id| {
                q.domains
                    .get(id)
                    .is_none_or(|d| d.phase() != phase || d.owner() != owner)
            }) {
                return Err("invalid checkpoint owner bucket".into());
            }
            // Admission records a bucket's orthant only for a full orthant;
            // the shortcut relies on it (A1), so a CP5 claiming another
            // domain is refused rather than trusted.
            if bucket
                .orthant
                .is_some_and(|id| !q.domains[id].is_full_orthant())
            {
                return Err("invalid checkpoint full-orthant ID".into());
            }
            bucket.indexed.validate(q.domains.len())?;
            if m.max_checks.is_some() && !bucket.indexed.is_empty() {
                return Err("checkpoint candidate index in the finite-cap lane".into());
            }
            if !keys.insert((phase, owner)) {
                return Err("duplicate checkpoint owner bucket".into());
            }
            stored.push(((phase, owner), bucket));
        }
        phases.since("index_owner_buckets", started);
        if m.max_checks.is_none() {
            let started = std::time::Instant::now();
            let indexed = indexed_ids(&q.domains, &stored)?;
            drop(indexed);
            q.restore_summaries()?;
            phases.since("index_summaries", started);
        }
        let started = std::time::Instant::now();
        q.by_owner
            .try_reserve(stored.len())
            .map_err(|_| "checkpoint owner bucket allocation")?;
        for (key, bucket) in stored {
            let (domains, summaries) = (&q.domains, &q.summaries);
            let indexed = index::AggregateIndex::restore(bucket.indexed, domains.len(), |id| {
                compact::stored_image(&domains[id], &summaries[id])
            })?;
            q.by_owner.insert(
                key,
                OwnerBucket {
                    ids: bucket.ids,
                    indexed,
                    orthant: bucket.orthant,
                },
            );
        }
        phases.since("index_blocks", started);
        let started = std::time::Instant::now();
        q.delegation = ledger
            .map(|l| l.restore(q.domains.iter().map(|d| (d.phase(), d.owner()))))
            .transpose()?;
        phases.since("ledger_restore", started);
        if q.delegation.as_ref().is_some_and(|l| l.cursor() != m.next) {
            return Err("checkpoint queue/ledger cursor mismatch".into());
        }
        q.next = m.next;
        q.deduplicated = m.deduplicated;
        q.containment_checks = m.containment_checks;
        q.containment_maintenance_checks = m.containment_maintenance_checks;
        q.containment_retired_candidates = m.containment_retired_candidates;
        q.containment_summary_builds = m.containment_summary_builds;
        q.containment_semantic_hits = m.containment_semantic_hits;
        q.containment_semantic_retirements = m.containment_semantic_retirements;
        q.exact_hits = m.exact_hits;
        q.orthant_hits = m.orthant_hits;
        q.max_finite_rank = m.max_finite_rank;
        q.unbounded_rank_domains = m.unbounded_rank_domains;
        Ok(q)
    }
}
/// Which IDs are indexed candidates. Each appears once, in its own
/// (phase, owner) bucket, so a repeated or misplaced ID is refused here, never
/// met mid-walk. Index positions are already range-checked against the domains.
fn indexed_ids<const N: usize>(
    domains: &[CompactDomain<N>],
    buckets: &[((Phase, [bool; N]), StoredOwnerBucket)],
) -> Result<Vec<bool>, String> {
    let mut indexed = Vec::new();
    indexed
        .try_reserve_exact(domains.len())
        .map_err(|_| "checkpoint index allocation")?;
    indexed.resize(domains.len(), false);
    let (mut repeated, mut misplaced) = (false, false);
    for ((phase, owner), bucket) in buckets {
        bucket.indexed.for_each_id(|id| {
            repeated |= std::mem::replace(&mut indexed[id], true);
            let domain = &domains[id];
            misplaced |= domain.phase() != *phase || domain.owner() != *owner;
        });
    }
    if repeated {
        return Err("duplicate checkpoint index ID".into());
    }
    if misplaced {
        return Err("invalid checkpoint owner bucket".into());
    }
    Ok(indexed)
}
impl<const N: usize> Queue<N> {
    /// Rebuild the unlimited lane's immutable per-ID summaries (every
    /// admitted ID, live or retired); the index blocks derive their words
    /// and lanes from them.
    fn restore_summaries(&mut self) -> Result<(), String> {
        self.summaries
            .try_reserve_exact(self.domains.len())
            .map_err(|_| "checkpoint summary allocation")?;
        for domain in &self.domains {
            let summary = domain.try_native_summary().map_err(|e| e.to_string())?;
            self.encountered_rank
                .observe(super::super::rank_telemetry::RankExtent::from_core(
                    &summary,
                ));
            self.summaries.push(CompactSummary::from_core(&summary));
        }
        Ok(())
    }
}
/// Whole-queue JSON image for tests and fixtures; production checkpoints
/// write the parts above as separate sections.
impl<const N: usize> Serialize for Queue<N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (
            self.checkpoint_metadata(),
            Domains(&self.domains),
            self.checkpoint_buckets(),
            self.checkpoint_ledger(),
        )
            .serialize(s)
    }
}
/// Decoded whole-queue JSON image.
type Image<const N: usize> = (
    Metadata,
    Vec<Domain<N>>,
    StoredBuckets,
    Option<StoredLedger>,
);
impl<const N: usize> Queue<N> {
    /// Restore a whole-queue image into `exact` (see `restore_with_index`).
    pub(super) fn restore_image(image: Image<N>, exact: ExactIndex<N>) -> Result<Self, String> {
        let (m, domains, buckets, ledger) = image;
        let domains = domains
            .iter()
            .map(CompactDomain::restore)
            .collect::<Result<_, _>>()?;
        Self::restore_with_index(
            m,
            domains,
            buckets,
            ledger,
            exact,
            false,
            &mut Phases::default(),
        )
    }
}
impl<'de, const N: usize> Deserialize<'de> for Queue<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::restore_image(Deserialize::deserialize(d)?, ExactIndex::new())
            .map_err(serde::de::Error::custom)
    }
}
