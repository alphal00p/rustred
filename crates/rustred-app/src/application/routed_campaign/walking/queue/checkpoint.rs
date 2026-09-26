//! Streaming queue image. Index membership/order is preserved, not re-admitted.
//! CP5 persists the parts separately: scalar metadata (JSON), immutable domain
//! records (append-only segments), owner buckets sorted by (phase, owner) so
//! their bytes are deterministic, and the responsibility ledger.
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
        Vec::<bool>::deserialize(d)?
            .try_into()
            .map_err(|_| serde::de::Error::custom("checkpoint owner arity"))
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
struct Domains<'a, const N: usize>(&'a [Arc<Domain<N>>]);
impl<const N: usize> Serialize for Domains<'_, N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for domain in self.0 {
            seq.serialize_element(domain.as_ref())?;
        }
        seq.end()
    }
}
/// Owner buckets in (phase, owner) order: identical queue state yields
/// identical bytes regardless of hash-map iteration order.
pub(in super::super) struct SortedBuckets<'a, const N: usize>(
    Vec<(&'a (Phase, [bool; N]), &'a OwnerBucket)>,
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
/// Decoded bucket image awaiting validation against the restored domains.
#[derive(Serialize, Deserialize)]
pub(in super::super) struct StoredBuckets(Vec<(Phase, Vec<bool>, OwnerBucket)>);
impl StoredBuckets {
    pub fn len(&self) -> usize {
        self.0.len()
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
    pub(in super::super) fn restore_from_parts(
        m: Metadata,
        domains: Vec<Domain<N>>,
        buckets: StoredBuckets,
        ledger: Option<StoredLedger>,
    ) -> Result<Self, String> {
        if m.next > domains.len()
            || domains.len() > m.max_domains
            || m.containment_retired_candidates > domains.len()
        {
            return Err("invalid checkpoint queue counters".into());
        }
        let mut q = Queue::new(m.max_domains, m.max_checks);
        q.domains
            .try_reserve_exact(domains.len())
            .map_err(|_| "checkpoint domain allocation")?;
        q.exact
            .try_reserve(domains.len())
            .map_err(|_| "checkpoint exact index allocation")?;
        for (id, domain) in domains.into_iter().enumerate() {
            if domain.lower.len() != N || domain.upper.len() != N {
                return Err("checkpoint coordinate arity".into());
            }
            domain.powers.validate().map_err(|e| e.to_string())?;
            if m.max_checks.is_none() {
                q.summaries.push(
                    DomainPowerSummary::try_new(
                        domain.owner,
                        &domain.lower,
                        &domain.upper,
                        domain.rank,
                        domain.powers,
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
            let domain = Arc::new(domain);
            if q.exact.insert(domain.clone(), id).is_some() {
                return Err("duplicate checkpoint exact domain".into());
            }
            q.domains.push(domain);
        }
        for (phase, owner, mut bucket) in buckets.0 {
            let owner: [bool; N] = owner.try_into().map_err(|_| "checkpoint bucket arity")?;
            if bucket.ids.iter().chain(bucket.orthant.iter()).any(|&id| {
                q.domains
                    .get(id)
                    .is_none_or(|d| d.phase != phase || d.owner != owner)
            }) {
                return Err("invalid checkpoint owner bucket".into());
            }
            bucket.indexed.restore_positions(q.domains.len())?;
            if q.by_owner.insert((phase, owner), bucket).is_some() {
                return Err("duplicate checkpoint owner bucket".into());
            }
        }
        q.delegation = ledger
            .map(|l| l.restore(q.domains.iter().map(|d| (d.phase, d.owner))))
            .transpose()?;
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
impl<'de, const N: usize> Deserialize<'de> for Queue<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (m, domains, buckets, ledger): (
            Metadata,
            Vec<Domain<N>>,
            StoredBuckets,
            Option<StoredLedger>,
        ) = Deserialize::deserialize(d)?;
        Self::restore_from_parts(m, domains, buckets, ledger).map_err(serde::de::Error::custom)
    }
}
