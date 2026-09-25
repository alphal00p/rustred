//! Inclusion reuse for one immutable program snapshot, not solved-state reuse.
use super::delegation::{Ledger, SchedulingPolicy};
use rustred::solver::{DomainPowerBounds, DomainPowerError, DomainPowerSummary};
use std::collections::HashMap;
use std::sync::Arc;

mod index;
use index::{AggregateIndex, Coordinates, Signature};
mod checkpoint;
mod prepared;
pub(super) use prepared::PreparedAdmission;
use prepared::PreparedLookup;

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
    /// historical scan and to measure additional semantic reuse.
    fn contains(&self, other: &Self) -> bool {
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

fn rank_contains(container: Option<u32>, candidate: Option<u32>) -> bool {
    container.is_none_or(|r| candidate.is_some_and(|s| s <= r))
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct OwnerBucket {
    /// Historical stable full scan, used only in the finite-cap lane.
    ids: Vec<usize>,
    /// Unlimited lane: grouped maximal lookup candidates. Retirement never
    /// removes the exact key, immutable domain or queued work.
    indexed: AggregateIndex,
    /// Largest admitted full-orthant rank; None rank dominates every finite R.
    orthant: Option<usize>,
}

#[cfg(test)]
impl OwnerBucket {
    fn candidate_ids(&self) -> Vec<usize> {
        let mut ids = self.indexed.ids();
        ids.extend_from_slice(&self.ids);
        ids.sort_unstable();
        ids
    }
}

pub(super) struct Queue<const N: usize> {
    /// Immutable storage shared with the exact index and an active inspection.
    /// Cloning a queued handle does not clone its coordinate vectors.
    pub domains: Vec<Arc<Domain<N>>>,
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
    exact: HashMap<Arc<Domain<N>>, usize>,
    by_owner: HashMap<(Phase, [bool; N]), OwnerBucket>,
    /// One immutable native summary per admitted ID in the unlimited lane.
    /// Raw domains, exact keys and scheduling obligations remain unchanged.
    summaries: Vec<DomainPowerSummary<N>>,
    max_domains: usize,
    max_checks: Option<usize>,
    /// Separates immutable lookup preparations from unrelated queue instances.
    identity: Arc<()>,
    /// Test instrumentation preference only, deliberately not persisted.
    #[cfg(test)]
    index_work_counters_enabled: bool,
}

impl<const N: usize> Queue<N> {
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
            exact: HashMap::new(),
            by_owner: HashMap::new(),
            summaries: Vec::new(),
            max_domains,
            max_checks,
            identity: Arc::new(()),
            #[cfg(test)]
            index_work_counters_enabled: true,
        }
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
        self.admit_with_lookup(domain, None)
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

    fn admit_with_lookup(
        &mut self,
        domain: Domain<N>,
        prepared: Option<PreparedLookup<N>>,
    ) -> Result<(usize, bool), &'static str> {
        debug_assert_eq!(domain.lower.len(), N);
        debug_assert_eq!(domain.upper.len(), N);
        // Borrowed full-domain lookup: hash collisions use full Eq, and no
        // coordinate vectors or Arc are allocated on this hot path.
        if let Some(&id) = self.exact.get(&domain) {
            self.exact_hits += 1;
            self.deduplicated += 1;
            return Ok((id, false));
        }
        let summary = if self.max_checks.is_none() {
            let builds = self
                .containment_summary_builds
                .checked_add(1)
                .ok_or("domain summary counter overflow")?;
            let summary = if let Some(prepared) = &prepared {
                prepared.summary.clone()
            } else {
                DomainPowerSummary::try_new(
                    domain.owner,
                    &domain.lower,
                    &domain.upper,
                    domain.rank,
                    domain.powers,
                )
                .map_err(summary_error)?
            };
            self.containment_summary_builds = builds;
            Some(summary)
        } else {
            None
        };
        let key = (domain.phase, domain.owner);
        if let Some(bucket) = self.by_owner.get(&key) {
            if let Some(id) = bucket.orthant
                && rank_contains(self.domains[id].rank, domain.rank)
            {
                self.orthant_hits += 1;
                self.deduplicated += 1;
                return Ok((id, false));
            }
            let found = if let Some(summary) = &summary {
                if let Some((found, checks)) = prepared.as_ref().and_then(|lookup| {
                    lookup.revalidate(&bucket.indexed, &self.summaries, self.containment_checks)
                }) {
                    self.containment_checks += checks; // checked by revalidate
                    found
                } else {
                    bucket
                        .indexed
                        .find(Signature::of(summary), Coordinates::of(summary), |id| {
                            self.containment_checks = self
                                .containment_checks
                                .checked_add(1)
                                .ok_or("domain containment counter overflow")?;
                            Ok(self.summaries[id].contains(summary))
                        })?
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
                    if self.domains[id].contains(&domain) {
                        found = Some(id);
                        break;
                    }
                }
                found
            };
            if let Some(id) = found {
                if summary.is_some() && !self.domains[id].contains(&domain) {
                    self.containment_semantic_hits = self
                        .containment_semantic_hits
                        .checked_add(1)
                        .ok_or("semantic containment hit counter overflow")?;
                }
                self.deduplicated += 1;
                return Ok((id, false));
            }
        }
        if self.domains.len() == self.max_domains {
            return Err("scheduled domain allowance");
        }
        let id = self.domains.len();
        if let Some(ledger) = &mut self.delegation {
            ledger
                .reserve_admission(id)
                .map_err(|_| "delegation ledger admission allocation")?;
        }
        self.domains
            .try_reserve(1)
            .map_err(|_| "domain allocation")?;
        self.exact
            .try_reserve(1)
            .map_err(|_| "exact domain index allocation")?;
        if summary.is_some() {
            self.summaries
                .try_reserve(1)
                .map_err(|_| "domain summary allocation")?;
        }
        // Reserve every fallible collection slot before publishing the domain,
        // either index, or rank telemetry. Failed reserves may change capacity,
        // never logical admission state. Occasional native HashMap rehash is
        // O(admitted domains); hash iteration never determines queue semantics.
        let signature = summary.as_ref().map(Signature::of);
        let coordinates = summary.as_ref().and_then(Coordinates::of);
        let (new_bucket, insertion) = if let Some(bucket) = self.by_owner.get_mut(&key) {
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
        let full_orthant = domain.is_full_orthant();
        // Preflight all reverse comparisons before changing the candidate
        // index or publishing this admission. A failed allocation above or a
        // counter overflow here can only alter reserved capacity/work counters,
        // never retire an obligation's only indexed representative.
        let maintenance = if let Some(signature) = signature {
            self.by_owner
                .get(&key)
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
                .admit_reserved(id, key)
                .map_err(|_| "delegation ledger admission invariant")?;
        }
        let domain = Arc::new(domain);
        if let Some(bucket) = new_bucket {
            self.by_owner.insert(key, bucket);
        }
        let bucket = self.by_owner.get_mut(&key).expect("reserved owner bucket");
        let mut extra_retired = 0;
        let retired = if let Some(insertion) = insertion.as_ref() {
            // All retained candidates and the new domain belong to this same
            // immutable phase/owner snapshot. Transitivity preserves a retained
            // containing representative for every retired candidate. Keep the
            // old domains/exact entries. InspectAll retains every FIFO job;
            // optional transfer changes only untouched, unreserved responsibility.
            let summary = summary.as_ref().expect("unlimited lane summary");
            bucket.indexed.retire(insertion, coordinates, |old| {
                let retire = summary.contains(&self.summaries[old]);
                if retire && !domain.contains(&self.domains[old]) {
                    extra_retired += 1; // preflighted by the maintenance bound
                }
                if retire && let Some(ledger) = &mut self.delegation {
                    // Same immutable phase/owner bucket; exact native inclusion
                    // is the authority. Protected work remains a native obligation.
                    let _ = ledger.transfer_retired(old, id);
                }
                retire
            })
        } else {
            0
        };
        self.containment_checks = total_checks;
        self.containment_maintenance_checks = maintenance_checks;
        self.containment_retired_candidates += retired;
        self.containment_semantic_retirements += extra_retired;
        if let Some(insertion) = insertion {
            bucket.indexed.insert(insertion, id, coordinates);
        } else {
            bucket.ids.push(id);
        }
        if full_orthant
            && bucket
                .orthant
                .is_none_or(|old| rank_contains(domain.rank, self.domains[old].rank))
        {
            bucket.orthant = Some(id);
        }
        if let Some(rank) = domain.rank {
            self.max_finite_rank = Some(self.max_finite_rank.map_or(rank, |old| old.max(rank)));
        } else {
            self.unbounded_rank_domains += 1;
        }
        self.exact.insert(Arc::clone(&domain), id);
        self.domains.push(domain);
        if let Some(summary) = summary {
            self.summaries.push(summary);
        }
        Ok((id, true))
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
