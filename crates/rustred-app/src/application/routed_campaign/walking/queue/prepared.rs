//! Read-only speculative lookup; ordered admission alone mutates the queue.
//!
//! # Prepared reverse retirement produces the serial result
//!
//! A helper prepares one proposal against an immutable snapshot S of the queue
//! whose live candidate set is L(S) and whose next admission ID is the
//! watermark W(S) = `domains.len()` at S. Ordered commit later applies the same
//! proposal to the queue in state T. Between S and T only earlier proposals of
//! the same batch committed, so L(T) = (L(S) − R) ∪ N where R are the IDs those
//! commits retired and N = {W(S), …} the IDs they admitted.
//!
//! Containment is pure geometry on immutable summaries: for every ID x,
//! `new.contains(summaries[x])` has the same value at S and at T, and the bit
//! word of x is a function of its summary. The prepared set P is
//! {x ∈ L(S) : new ⊇ x} restricted to the group/block-eligible candidates of
//! S. Group signatures are immutable, an ID never changes group or block, and a
//! block envelope only ever widens (insertion) or stays (retain), so any
//! candidate that a snapshot filter skipped is provably not contained, and any
//! candidate the commit-time filter admits is either in P or provably not
//! contained. Hence for every old ID x < W(S) that `retire` examines at T,
//! `x ∈ P ⇔ new ⊇ x`. IDs in R are absent from every block at T, so retain
//! never evaluates them on either path. IDs in N are decided at T by
//! `contains_new`, exactly as the serial callback would. `retire_prepared` is
//! `retire` with this composite predicate, so it walks the same groups, blocks
//! and IDs in the same order, retains the same members, pins the same tail and
//! removes the same empty groups: the resulting index layout and the retired
//! count are identical. Every per-retirement effect (`extra_retired`, the
//! ledger's `transfer_retired`) is evaluated at T inside the retirement loop on
//! both paths, so transfers and semantic-retirement counters agree; the
//! `containment_checks` and `containment_maintenance_checks` charges are the
//! commit-time `maintenance_len` bound on both paths. A snapshot hit that is
//! still live never retires and needs no set; a retired winner and a
//! near-exhausted counter fall back to the full serial path as before.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Largest reverse set a helper may prepare; beyond it the commit retires on
/// the serial path. Bounds helper memory per in-flight proposal.
pub(in super::super) const PREPARED_RETIRE_LIMIT: usize = 65_536;

/// Bounded-batch worker result, inseparably bound to its exact proposed domain.
/// This is lookup evidence, not admission or completion of pending work.
pub(in super::super) struct PreparedAdmission<const N: usize> {
    domain: Domain<N>,
    identity: Arc<()>,
    lookup: Option<PreparedLookup<N>>,
    work: SpeculativeWork,
}

/// All completed helper-side native comparisons and bit rejections during one
/// preparation, even if its evidence is later cancelled, stale, discarded or
/// bypassed by exact reuse. Separate from ordered queue-prefix accounting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct SpeculativeWork {
    pub checks: usize,
    pub reverse_checks: usize,
    pub forward_bit_rejections: usize,
    pub reverse_bit_rejections: usize,
}

impl<const N: usize> PreparedAdmission<N> {
    /// All completed forward native containment calls during preparation.
    pub fn speculative_checks(&self) -> usize {
        self.work.checks
    }

    pub fn speculative_work(&self) -> SpeculativeWork {
        self.work
    }

    #[cfg(test)]
    pub(super) fn prepared_retire_len(&self) -> Option<usize> {
        self.lookup
            .as_ref()
            .and_then(|lookup| lookup.retire.as_ref().map(Vec::len))
    }

    #[cfg(test)]
    pub(super) fn has_lookup(&self) -> bool {
        self.lookup.is_some()
    }
}

pub(super) struct PreparedLookup<const N: usize> {
    pub(super) summary: DomainPowerSummary<N>,
    word: u64,
    watermark: usize,
    found: Option<usize>,
    checks: usize,
    /// Prepared reverse retirement set (ascending, all below the watermark),
    /// only for a snapshot miss and only while it fitted the limit.
    retire: Option<Vec<usize>>,
}

/// Outcome of revalidating a prepared lookup against the commit-time index.
pub(super) struct Revalidated {
    pub found: Option<usize>,
    /// Forward comparisons to charge, bounded before the scan.
    pub checks: usize,
    /// On a miss: the prepared reverse set to apply with `retire_prepared`.
    pub retire: Option<Vec<usize>>,
    /// IDs at or above this watermark are decided at commit time.
    pub first_new: usize,
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
        let mut work = SpeculativeWork::default();
        let lookup = self.prepare_lookup(&domain, &mut is_cancelled, &mut work);
        PreparedAdmission {
            domain,
            identity: Arc::clone(&self.identity),
            lookup,
            work,
        }
    }

    fn prepare_lookup(
        &self,
        domain: &Domain<N>,
        is_cancelled: &mut impl FnMut() -> bool,
        work: &mut SpeculativeWork,
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
        let word = bits::word(&summary);
        let prefilter = self.prefilter;
        let signature = Signature::of(&summary);
        let coordinates = Coordinates::of(&summary);
        let (found, retire) = if let Some(bucket) = self.by_owner.get(&(domain.phase, domain.owner))
        {
            if bucket
                .orthant
                .is_some_and(|id| rank_contains(self.domains[id].rank, domain.rank))
            {
                // Commit must still reproduce the ordinary summary preflight
                // and fresh exact/orthant priority, without a general scan.
                (None, None)
            } else {
                let checkpoint = || {
                    if is_cancelled() {
                        Err("cancelled speculative lookup")
                    } else {
                        Ok(())
                    }
                };
                let found = bucket
                    .indexed
                    .find_controlled(signature, coordinates, 0, checkpoint, |id| {
                        work.checks = work
                            .checks
                            .checked_add(1)
                            .ok_or("speculative check overflow")?;
                        let rejected = prefilter.rejects(self.bits[id], word);
                        work.forward_bit_rejections = work
                            .forward_bit_rejections
                            .saturating_add(usize::from(rejected));
                        Ok(!rejected && self.summaries[id].contains(&summary))
                    })
                    .ok()?;
                let retire = if found.is_none() {
                    // A miss commits a new candidate, so prepare the reverse
                    // pass too. Failure here only forfeits the prepared set;
                    // the forward evidence remains valid.
                    let checkpoint = || {
                        if is_cancelled() {
                            Err("cancelled speculative lookup")
                        } else {
                            Ok(())
                        }
                    };
                    bucket
                        .indexed
                        .collect_contained(
                            signature,
                            coordinates,
                            PREPARED_RETIRE_LIMIT,
                            checkpoint,
                            |id| {
                                work.reverse_checks = work
                                    .reverse_checks
                                    .checked_add(1)
                                    .ok_or("speculative check overflow")?;
                                let rejected = prefilter.rejects(word, self.bits[id]);
                                work.reverse_bit_rejections = work
                                    .reverse_bit_rejections
                                    .saturating_add(usize::from(rejected));
                                Ok(!rejected && summary.contains(&self.summaries[id]))
                            },
                        )
                        .ok()
                } else {
                    None
                };
                (found, retire)
            }
        } else {
            // No candidate of this phase/owner existed at the snapshot.
            (None, Some(Vec::new()))
        };
        if is_cancelled() {
            return None;
        }
        Some(PreparedLookup {
            summary,
            word,
            watermark: self.domains.len(),
            found,
            checks: work.checks,
            retire,
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn revalidate(
        &mut self,
        index: &AggregateIndex,
        summaries: &[DomainPowerSummary<N>],
        bits: &[u64],
        prefilter: bits::Prefilter,
        previous_checks: usize,
        session: &mut SessionCounters,
    ) -> Option<Revalidated> {
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
                .then_some(Revalidated {
                    found: Some(id),
                    checks: self.checks,
                    retire: None,
                    first_new: self.watermark,
                });
        }
        // Every old live ID was tested by the snapshot (or safely filtered).
        // Retirements remove choices; only subsequent admissions can add one.
        let mut checks = self.checks;
        let word = self.word;
        let found = index
            .find_from(
                Signature::of(&self.summary),
                Coordinates::of(&self.summary),
                self.watermark,
                |id| {
                    checks += 1; // bounded above before this scan
                    let rejected = prefilter.rejects(bits[id], word);
                    session.forward(rejected);
                    Ok(!rejected && summaries[id].contains(&self.summary))
                },
            )
            .ok()?;
        Some(Revalidated {
            retire: if found.is_none() {
                self.retire.take()
            } else {
                None
            },
            found,
            checks,
            first_new: self.watermark,
        })
    }
}
