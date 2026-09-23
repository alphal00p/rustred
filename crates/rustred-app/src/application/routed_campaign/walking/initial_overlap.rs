//! Optional exact reuse of a high-D slice of an immutable initial Apply domain.
//! Inclusion is native geometry; coverage remains a pinned ledger obligation.
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rustred::solver::{DomainPowerBounds, DomainPowerSummary};

use super::queue::{Domain, Phase};

pub(super) const MAX_INITIAL_DOMAINS: usize = 4096;
pub(super) const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;

/// Original coordinates/rank are unchanged. Only residual D bounds differ.
/// This is partial native work, never a whole-domain alias or a solved anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialOverlapScope {
    pub anchor_id: usize,
    pub cut: i64,
    pub residual_powers: DomainPowerBounds,
}

pub(super) struct InitialOverlapPlan<const N: usize> {
    pub scope: InitialOverlapScope,
    pub residual: Domain<N>,
}

struct Anchor<const N: usize> {
    id: usize,
    cut: i64,
    summary: DomainPowerSummary<N>,
}

pub(super) struct InitialOverlapIndex<const N: usize> {
    // Complete or empty: even initial entries omitted from the anchor lists
    // must bypass pruning. The persistent queue exact map forbids a later
    // identical raw domain from receiving a second ID.
    initial: HashSet<Arc<Domain<N>>>,
    anchors: HashMap<[bool; N], Vec<Anchor<N>>>,
}

impl<const N: usize> InitialOverlapIndex<N> {
    pub fn empty() -> Self {
        Self {
            initial: HashSet::new(),
            anchors: HashMap::new(),
        }
    }

    /// Caller has already pinned this actual initial prefix before its first
    /// admission and frozen it in the queue-local responsibility ledger.
    pub fn from_initial(domains: &[Arc<Domain<N>>], cancellation: &AtomicBool) -> Self {
        Self::with_limits(domains, cancellation, MAX_INITIAL_DOMAINS, MAX_ENTRY_BYTES)
    }

    fn with_limits(
        domains: &[Arc<Domain<N>>],
        cancellation: &AtomicBool,
        max_domains: usize,
        max_bytes: usize,
    ) -> Self {
        // Logical entry payload only, not allocator overhead or RSS. Domain
        // coordinate allocations are shared by Arc, never cloned into the index.
        let entry_bytes = size_of::<Arc<Domain<N>>>()
            + size_of::<Anchor<N>>()
            + size_of::<([bool; N], Vec<Anchor<N>>)>();
        if domains.len() > max_domains || domains.len() > max_bytes / entry_bytes {
            return Self::empty();
        }
        let mut out = Self::empty();
        if out.initial.try_reserve(domains.len()).is_err()
            || out.anchors.try_reserve(domains.len()).is_err()
        {
            return Self::empty();
        }
        for (id, domain) in domains.iter().enumerate() {
            if cancellation.load(Ordering::Acquire) {
                return Self::empty();
            }
            out.initial.insert(domain.clone());
            if domain.phase != Phase::Apply {
                continue;
            }
            let Ok(summary) = summary(domain, domain.powers) else {
                continue;
            };
            let Some(cut) = summary
                .extrema()
                .and_then(|x| x.power_difference().0)
                .and_then(|x| i64::try_from(x).ok())
            else {
                continue;
            };
            if cut.checked_sub(1).is_none() {
                continue;
            }
            let anchors = out.anchors.entry(domain.owner).or_default();
            if anchors.try_reserve(1).is_err() {
                return Self::empty();
            }
            anchors.push(Anchor { id, cut, summary });
        }
        out
    }

    /// Deterministic first native-valid cut in initial admission order.
    /// Optional failures never skip work: the caller inspects the original Q.
    pub fn plan(
        &self,
        domain: &Domain<N>,
        cancellation: &AtomicBool,
    ) -> Option<InitialOverlapPlan<N>> {
        if domain.phase != Phase::Apply || self.initial.contains(domain) {
            return None;
        }
        let anchors = self.anchors.get(&domain.owner)?;
        // Validate the unmodified descriptor first. An inverted generated
        // intersection is an empty split, not permission to mask bad input.
        if summary(domain, domain.powers).ok()?.is_empty() {
            return None;
        }
        for anchor in anchors {
            if cancellation.load(Ordering::Acquire) {
                return None;
            }
            let below = anchor.cut.checked_sub(1)?;
            let mut high = domain.powers;
            high.min_power_difference = Some(
                high.min_power_difference
                    .map_or(anchor.cut, |v| v.max(anchor.cut)),
            );
            let mut low = domain.powers;
            low.max_power_difference =
                Some(low.max_power_difference.map_or(below, |v| v.min(below)));
            if inverted(high) || inverted(low) {
                continue;
            }
            let (Ok(high_summary), Ok(low_summary)) = (summary(domain, high), summary(domain, low))
            else {
                continue;
            };
            if high_summary.is_empty()
                || low_summary.is_empty()
                || !anchor.summary.contains(&high_summary)
            {
                continue;
            }
            let mut lower = Vec::new();
            let mut upper = Vec::new();
            if lower.try_reserve_exact(domain.lower.len()).is_err()
                || upper.try_reserve_exact(domain.upper.len()).is_err()
            {
                return None;
            }
            lower.extend_from_slice(&domain.lower);
            upper.extend_from_slice(&domain.upper);
            return Some(InitialOverlapPlan {
                scope: InitialOverlapScope {
                    anchor_id: anchor.id,
                    cut: anchor.cut,
                    residual_powers: low,
                },
                residual: Domain {
                    phase: domain.phase,
                    owner: domain.owner,
                    lower,
                    upper,
                    rank: domain.rank,
                    powers: low,
                },
            });
        }
        None
    }
}

fn inverted(bounds: DomainPowerBounds) -> bool {
    matches!((bounds.min_power_difference, bounds.max_power_difference), (Some(a), Some(b)) if a > b)
}

fn summary<const N: usize>(
    domain: &Domain<N>,
    powers: DomainPowerBounds,
) -> Result<DomainPowerSummary<N>, rustred::solver::DomainPowerError> {
    DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        powers,
    )
}

#[cfg(test)]
mod tests;
