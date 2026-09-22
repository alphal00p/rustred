//! Inclusion reuse for one immutable program snapshot, not solved-state reuse.
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Phase {
    Apply,
    Route,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Domain<const N: usize> {
    pub phase: Phase,
    pub owner: [bool; N],
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
}

impl<const N: usize> Domain<N> {
    /// Prospective source orthant for the admitted-route rank bound. Enclosed
    /// points are not necessarily reached by nonzero coefficients.
    pub fn route_cover(owner: [bool; N], rank: Option<u32>) -> Self {
        Self {
            phase: Phase::Route,
            owner,
            rank,
            lower: vec![0; N],
            upper: vec![None; N],
        }
    }

    fn contains(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.owner == other.owner
            && rank_contains(self.rank, other.rank)
            && self.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            && self
                .upper
                .iter()
                .zip(&other.upper)
                .all(|(a, b)| a.is_none_or(|a| b.is_some_and(|b| b <= a)))
    }

    fn is_full_orthant(&self) -> bool {
        self.lower.len() == N
            && self.upper.len() == N
            && self.lower.iter().all(|&x| x == 0)
            && self.upper.iter().all(Option::is_none)
    }
}

fn rank_contains(container: Option<u32>, candidate: Option<u32>) -> bool {
    container.is_none_or(|r| candidate.is_some_and(|s| s <= r))
}

#[derive(Default)]
struct OwnerBucket {
    /// Original admission order for the unchanged arbitrary-box fallback.
    ids: Vec<usize>,
    /// Largest admitted full-orthant rank; None rank dominates every finite R.
    orthant: Option<usize>,
}

pub(super) struct Queue<const N: usize> {
    /// Immutable storage shared with the exact index and an active inspection.
    /// Cloning a queued handle does not clone its coordinate vectors.
    pub domains: Vec<Arc<Domain<N>>>,
    pub next: usize,
    pub deduplicated: usize,
    /// General box comparisons only, not hash equality or indexed rank checks.
    pub containment_checks: usize,
    pub exact_hits: usize,
    pub orthant_hits: usize,
    pub max_finite_rank: Option<u32>,
    pub unbounded_rank_domains: usize,
    exact: HashMap<Arc<Domain<N>>, usize>,
    by_owner: HashMap<(Phase, [bool; N]), OwnerBucket>,
    max_domains: usize,
    max_checks: Option<usize>,
}

impl<const N: usize> Queue<N> {
    pub fn containment_limit(&self) -> Option<usize> {
        self.max_checks
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
            next: 0,
            deduplicated: 0,
            containment_checks: 0,
            exact_hits: 0,
            orthant_hits: 0,
            max_finite_rank: None,
            unbounded_rank_domains: 0,
            exact: HashMap::new(),
            by_owner: HashMap::new(),
            max_domains,
            max_checks,
        }
    }

    /// A pending containing domain can suppress another scheduling request, but
    /// every admitted domain still has to finish before worklist exhaustion.
    /// The queue is never shared between different snapshots or rank policies.
    /// Exact/full-orthant index proofs do not spend general containment checks,
    /// so they can still succeed at a finite comparison cap or counter maximum.
    /// None means no policy cap, but counter overflow remains an explicit error.
    /// A dominant orthant can
    /// return a different valid containing ID than the legacy first-match scan;
    /// new-domain IDs/FIFO order remain unchanged with unlimited comparisons.
    pub fn admit(&mut self, domain: Domain<N>) -> Result<(usize, bool), &'static str> {
        debug_assert_eq!(domain.lower.len(), N);
        debug_assert_eq!(domain.upper.len(), N);
        // Borrowed full-domain lookup: hash collisions use full Eq, and no
        // coordinate vectors or Arc are allocated on this hot path.
        if let Some(&id) = self.exact.get(&domain) {
            self.exact_hits += 1;
            self.deduplicated += 1;
            return Ok((id, false));
        }
        let key = (domain.phase, domain.owner);
        if let Some(bucket) = self.by_owner.get(&key) {
            if let Some(id) = bucket.orthant
                && rank_contains(self.domains[id].rank, domain.rank)
            {
                self.orthant_hits += 1;
                self.deduplicated += 1;
                return Ok((id, false));
            }
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
                    self.deduplicated += 1;
                    return Ok((id, false));
                }
            }
        }
        if self.domains.len() == self.max_domains {
            return Err("scheduled domain allowance");
        }
        let id = self.domains.len();
        self.domains
            .try_reserve(1)
            .map_err(|_| "domain allocation")?;
        self.exact
            .try_reserve(1)
            .map_err(|_| "exact domain index allocation")?;
        // Reserve every fallible collection slot before publishing the domain,
        // either index, or rank telemetry. Failed reserves may change capacity,
        // never logical admission state. Occasional native HashMap rehash is
        // O(admitted domains); hash iteration never determines queue semantics.
        let new_bucket = if let Some(bucket) = self.by_owner.get_mut(&key) {
            bucket
                .ids
                .try_reserve(1)
                .map_err(|_| "owner domain index allocation")?;
            None
        } else {
            self.by_owner
                .try_reserve(1)
                .map_err(|_| "owner domain index allocation")?;
            let mut bucket = OwnerBucket::default();
            bucket
                .ids
                .try_reserve(1)
                .map_err(|_| "owner domain index allocation")?;
            Some(bucket)
        };
        let full_orthant = domain.is_full_orthant();
        let domain = Arc::new(domain);
        if let Some(bucket) = new_bucket {
            self.by_owner.insert(key, bucket);
        }
        let bucket = self.by_owner.get_mut(&key).expect("reserved owner bucket");
        bucket.ids.push(id);
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
        Ok((id, true))
    }
}

#[cfg(test)]
mod tests;
