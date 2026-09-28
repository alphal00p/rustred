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
//!
//! Summaries are immutable per ID and never released, so a snapshot winner
//! retired since S still has its summary: its signature is read and the
//! `is_live == false` fallback decides.
//!
//! The struct-of-arrays kernel keeps these properties: its words and lanes
//! are functions of the immutable summaries and only reject candidates that
//! the exact predicate would reject, so the prepared sets, the forward scans
//! and every charged candidate are those of the per-ID callback scan.

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
    /// The compact image and exact-index digest of `domain`, computed on the
    /// helper so the ordered commit does not hash it again. None only for a
    /// domain outside the compact range (the commit then refuses it).
    key: Option<(CompactDomain<N>, Digest)>,
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
    /// Candidates that reached the exact predicate (the rest were rejected
    /// by the bit word or the u8 lanes), and the index scans' wall time.
    pub forward_tests: usize,
    pub reverse_tests: usize,
    pub scan_nanos: u64,
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
    pub(super) query: Query<N>,
    watermark: usize,
    found: Option<usize>,
    checks: usize,
    /// Prepared reverse retirement set (ascending, all below the watermark),
    /// only for a snapshot miss and only while it fitted the limit.
    retire: Option<Vec<usize>>,
    /// No candidate of this phase/owner existed at the snapshot: the empty
    /// set above decides nothing, every retirement is a commit-time decision.
    bucket_absent: bool,
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
    /// The set is empty because the bucket did not exist at the snapshot.
    pub trivial: bool,
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
        let key = CompactDomain::try_from_domain(&domain)
            .ok()
            .map(|compact| (compact, self.exact.key(&compact)));
        let lookup = self.prepare_lookup(&domain, key.as_ref(), &mut is_cancelled, &mut work);
        PreparedAdmission {
            domain,
            identity: Arc::clone(&self.identity),
            key,
            lookup,
            work,
        }
    }

    fn prepare_lookup(
        &self,
        domain: &Domain<N>,
        key: Option<&(CompactDomain<N>, Digest)>,
        is_cancelled: &mut impl FnMut() -> bool,
        work: &mut SpeculativeWork,
    ) -> Option<PreparedLookup<N>> {
        if self.max_checks.is_some() || is_cancelled() {
            return None;
        }
        // An unrepresentable domain is refused by the ordered commit itself.
        let &(compact, key) = key?;
        if self
            .exact
            .get_live(key, &compact, &self.domains, &self.quarantine)
            .is_ok()
        {
            return None;
        }
        let query = Query::new(
            DomainPowerSummary::try_new(
                domain.owner,
                &domain.lower,
                &domain.upper,
                domain.rank,
                domain.powers,
            )
            .ok()?,
            domain.phase,
        );
        let stored = self.stored();
        let signature = Signature::of(&query.core);
        let probe = self.probe(&query);
        let started = std::time::Instant::now();
        let bucket = self.by_owner.get(&(domain.phase, domain.owner));
        let bucket_absent = bucket.is_none();
        let (found, retire) = if let Some(bucket) = bucket {
            // The commit's own shortcut predicate. A later orthant of the
            // bucket dominates this one's rank, so the ordered commit returns
            // an orthant hit too (or an exact hit, checked before it).
            if bucket
                .orthant_hit(
                    &self.domains,
                    &self.quarantine,
                    compact.bucket_code(),
                    domain.rank,
                )
                .is_some()
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
                    .find_controlled(
                        signature,
                        &probe,
                        0,
                        checkpoint,
                        &mut Speculative {
                            work: &mut *work,
                            reverse: false,
                            stored,
                            query: &query,
                        },
                    )
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
                            &probe,
                            PREPARED_RETIRE_LIMIT,
                            checkpoint,
                            &mut Speculative {
                                work: &mut *work,
                                reverse: true,
                                stored,
                                query: &query,
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
        drop(probe);
        work.scan_nanos = work
            .scan_nanos
            .saturating_add(started.elapsed().as_nanos() as u64);
        if is_cancelled() {
            return None;
        }
        Some(PreparedLookup {
            query,
            watermark: self.domains.len(),
            found,
            checks: work.checks,
            retire,
            bucket_absent,
        })
    }

    /// Commit in the original proposal order. A stale preparation can avoid
    /// old comparisons, never avoid fresh exact/orthant checks, reverse
    /// maintenance, limits, reservations or pending-work publication.
    pub fn admit_prepared(
        &mut self,
        prepared: PreparedAdmission<N>,
    ) -> Result<(usize, bool), &'static str> {
        let same_queue = Arc::ptr_eq(&self.identity, &prepared.identity);
        let lookup = if same_queue && self.max_checks.is_none() {
            prepared
                .lookup
                .filter(|lookup| lookup.watermark <= self.domains.len())
        } else {
            None
        };
        // The digest depends only on the domain and this queue's key function.
        let key = prepared.key.filter(|_| same_queue);
        self.admit_with_lookup(prepared.domain, lookup, key)
    }
}

impl<const N: usize> PreparedLookup<N> {
    /// Return None to use the original serial lookup. In particular, overflow
    /// risk from discarded/stale speculative comparisons must not move a
    /// public failure earlier than ordinary lookup would have produced it.
    pub(super) fn revalidate(
        &mut self,
        index: &AggregateIndex<N>,
        stored: Stored<'_, N>,
        filter: bool,
        previous_checks: usize,
        session: &mut SessionCounters,
    ) -> Option<Revalidated> {
        // Group traversal order may change after retirement. A fresh forward
        // scan can do more OR less work than its prepared counterpart, and a
        // miss also performs reverse maintenance after this method returns.
        // Near counter exhaustion, preserve the exact old failure prefix by
        // falling back unless both complete alternatives fit without overflow.
        // The bound counts every admitted ID.
        previous_checks
            .checked_add(self.checks)?
            .checked_add(stored.summaries.len().checked_mul(2)?)?;
        if let Some(id) = self.found {
            // Snapshot misses before this minimum remain misses. New IDs are
            // greater, and other retirements cannot introduce an earlier hit.
            // A winner retired since the snapshot keeps its immutable summary;
            // `is_live == false` falls back to the serial scan.
            return index
                .is_live(stored.signature(id), id)
                .then_some(Revalidated {
                    found: Some(id),
                    checks: self.checks,
                    retire: None,
                    first_new: self.watermark,
                    trivial: false,
                });
        }
        // Every old live ID was tested by the snapshot (or safely filtered).
        // Retirements remove choices; only subsequent admissions can add one.
        let mut checks = self.checks;
        let query = &self.query;
        let probe = Probe::new(
            Coordinates::of(&query.core),
            query.word,
            query.lanes,
            filter,
        );
        let started = std::time::Instant::now();
        let found = index
            .find_from(
                Signature::of(&query.core),
                &probe,
                self.watermark,
                &mut Revalidation {
                    checks: &mut checks,
                    session: &mut *session,
                    stored,
                    query,
                },
            )
            .ok()?;
        session.forward_scan(started);
        Some(Revalidated {
            retire: if found.is_none() {
                self.retire.take()
            } else {
                None
            },
            found,
            checks,
            first_new: self.watermark,
            trivial: self.bucket_absent,
        })
    }
}

/// Helper-side visitor: every completed forward or reverse callback of a
/// speculative preparation, rejections included.
struct Speculative<'a, const N: usize> {
    work: &'a mut SpeculativeWork,
    reverse: bool,
    stored: Stored<'a, N>,
    query: &'a Query<N>,
}

impl<const N: usize> Speculative<'_, N> {
    fn charge(&mut self, count: usize, words: usize) -> Result<(), &'static str> {
        let (checks, rejections) = if self.reverse {
            (
                &mut self.work.reverse_checks,
                &mut self.work.reverse_bit_rejections,
            )
        } else {
            (&mut self.work.checks, &mut self.work.forward_bit_rejections)
        };
        *checks = checks
            .checked_add(count)
            .ok_or("speculative check overflow")?;
        *rejections = rejections.saturating_add(words);
        Ok(())
    }
}

impl<const N: usize> Visit for Speculative<'_, N> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        self.charge(run.len(), word.count_ones() as usize)
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        self.charge(1, 0)?;
        let tests = if self.reverse {
            &mut self.work.reverse_tests
        } else {
            &mut self.work.forward_tests
        };
        *tests = tests.saturating_add(1);
        Ok(if self.reverse {
            self.stored.contained_by(id, self.query)
        } else {
            self.stored.contains(id, self.query)
        })
    }
}

/// Commit-time forward scan above the snapshot watermark.
struct Revalidation<'a, const N: usize> {
    checks: &'a mut usize,
    session: &'a mut SessionCounters,
    stored: Stored<'a, N>,
    query: &'a Query<N>,
}

impl<const N: usize> Visit for Revalidation<'_, N> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        *self.checks += run.len(); // bounded above before this scan
        self.session
            .forward_run(run.len(), word.count_ones() as usize);
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        *self.checks += 1;
        self.session.forward_test();
        Ok(self.stored.contains(id, self.query))
    }
}
