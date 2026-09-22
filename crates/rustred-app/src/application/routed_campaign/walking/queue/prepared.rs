//! Read-only speculative lookup; ordered admission alone mutates the queue.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Bounded-batch worker result, inseparably bound to its exact proposed domain.
/// This is lookup evidence, not admission or completion of pending work.
pub(in super::super) struct PreparedAdmission<const N: usize> {
    domain: Domain<N>,
    identity: Arc<()>,
    lookup: Option<PreparedLookup<N>>,
    speculative_checks: usize,
}

impl<const N: usize> PreparedAdmission<N> {
    /// All completed native containment calls during preparation, even if its
    /// evidence is later cancelled, stale, discarded or bypassed by exact reuse.
    /// Separate from ordered queue-prefix work accounting.
    pub fn speculative_checks(&self) -> usize {
        self.speculative_checks
    }
}

pub(super) struct PreparedLookup<const N: usize> {
    pub(super) summary: DomainPowerSummary<N>,
    watermark: usize,
    found: Option<usize>,
    checks: usize,
}

impl<const N: usize> Queue<N> {
    /// Call concurrently through an immutable queue reference. The caller owns
    /// the bounded batch and worker budget; no queue state or public accounting
    /// changes here. Errors and cancellation discard speculation and are never
    /// exposed ahead of the proposal's position in the serial admission stream.
    pub fn prepare_admission(
        &self,
        domain: Domain<N>,
        cancellation: &AtomicBool,
    ) -> PreparedAdmission<N> {
        self.prepare_admission_check(domain, || cancellation.load(Ordering::Relaxed))
    }

    /// Also respond to the producer's native-failure/stop signal without
    /// introducing another watcher thread or changing ordered error authority.
    pub fn prepare_admission_with_stop(
        &self,
        domain: Domain<N>,
        cancellation: &AtomicBool,
        producer_stop: &AtomicBool,
    ) -> PreparedAdmission<N> {
        self.prepare_admission_check(domain, || {
            cancellation.load(Ordering::Relaxed) || producer_stop.load(Ordering::Relaxed)
        })
    }

    pub(super) fn prepare_admission_check(
        &self,
        domain: Domain<N>,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> PreparedAdmission<N> {
        let mut speculative_checks = 0;
        let lookup = self.prepare_lookup(&domain, &mut is_cancelled, &mut speculative_checks);
        PreparedAdmission {
            domain,
            identity: Arc::clone(&self.identity),
            lookup,
            speculative_checks,
        }
    }

    fn prepare_lookup(
        &self,
        domain: &Domain<N>,
        is_cancelled: &mut impl FnMut() -> bool,
        checks: &mut usize,
    ) -> Option<PreparedLookup<N>> {
        if self.max_checks.is_some() || is_cancelled() || self.exact.contains_key(domain) {
            return None;
        }
        let summary = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )
        .ok()?;
        let found = if let Some(bucket) = self.by_owner.get(&(domain.phase, domain.owner)) {
            if bucket
                .orthant
                .is_some_and(|id| rank_contains(self.domains[id].rank, domain.rank))
            {
                // Commit must still reproduce the ordinary summary preflight
                // and fresh exact/orthant priority, without a general scan.
                None
            } else {
                bucket
                    .indexed
                    .find(Signature::of(&summary), |id| {
                        if is_cancelled() {
                            return Err("cancelled speculative lookup");
                        }
                        *checks = checks.checked_add(1).ok_or("speculative check overflow")?;
                        Ok(self.summaries[id].contains(&summary))
                    })
                    .ok()?
            }
        } else {
            None
        };
        if is_cancelled() {
            return None;
        }
        Some(PreparedLookup {
            summary,
            watermark: self.domains.len(),
            found,
            checks: *checks,
        })
    }

    /// Commit in the original proposal order. A stale preparation can avoid
    /// old comparisons, never avoid fresh exact/orthant checks, reverse
    /// maintenance, limits, reservations or pending-work publication.
    pub fn admit_prepared(
        &mut self,
        prepared: PreparedAdmission<N>,
    ) -> Result<(usize, bool), &'static str> {
        let lookup = if Arc::ptr_eq(&self.identity, &prepared.identity) && self.max_checks.is_none()
        {
            prepared
                .lookup
                .filter(|lookup| lookup.watermark <= self.domains.len())
        } else {
            None
        };
        self.admit_with_lookup(prepared.domain, lookup)
    }
}

impl<const N: usize> PreparedLookup<N> {
    /// Return None to use the original serial lookup. In particular, overflow
    /// risk from discarded/stale speculative comparisons must not move a
    /// public failure earlier than ordinary lookup would have produced it.
    pub(super) fn revalidate(
        &self,
        index: &AggregateIndex,
        summaries: &[DomainPowerSummary<N>],
        previous_checks: usize,
    ) -> Option<(Option<usize>, usize)> {
        // Group traversal order may change after retirement. A fresh forward
        // scan can do more OR less work than its prepared counterpart, and a
        // miss also performs reverse maintenance after this method returns.
        // Near counter exhaustion, preserve the exact old failure prefix by
        // falling back unless both complete alternatives fit without overflow.
        previous_checks
            .checked_add(self.checks)?
            .checked_add(summaries.len().checked_mul(2)?)?;
        if let Some(id) = self.found {
            // Snapshot misses before this minimum remain misses. New IDs are
            // greater, and other retirements cannot introduce an earlier hit.
            return index
                .is_live(Signature::of(&summaries[id]), id)
                .then_some((Some(id), self.checks));
        }
        // Every old live ID was tested by the snapshot (or safely filtered).
        // Retirements remove choices; only subsequent admissions can add one.
        let mut checks = self.checks;
        let found = index
            .find_from(Signature::of(&self.summary), self.watermark, |id| {
                checks += 1; // bounded above before this scan
                Ok(summaries[id].contains(&self.summary))
            })
            .ok()?;
        Some((found, checks))
    }
}
