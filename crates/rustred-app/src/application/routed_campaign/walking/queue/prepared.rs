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
//! Summaries sit in reusable slab slots (`compact::SummarySlab`): retirement
//! releases a candidate's slot and a later admission may overwrite it. Every
//! comparison above reads live candidates only (IDs present in the index at
//! the time of the read), whose slots are never released underneath them;
//! the one ID that may have been retired since S, a snapshot winner, is
//! checked for a released slot before its signature is read, which is
//! exactly the `is_live == false` fallback.
//!
//! # Certified helper verdicts produce the serial early return
//!
//! Most requests hit at S, and the helper's verdict is then usually final.
//! `certify` applies it in O(1) only when `admit_with_lookup` at T provably
//! takes the same early return with the same ID, and then applies the same
//! counter increments in the same order:
//!
//! - Exact hit e at S. The exact index is append-only and holds each domain
//!   once, so the confirmed exact ID at T is e. Nothing else is needed.
//! - Full-orthant hit o at S, or containment winner f at S (the minimum live
//!   containing ID). Both need (a) no exact hit at T and (b) `bucket.orthant`
//!   unchanged since S. The exact index and `bucket.orthant` change only by
//!   admissions, and the admissions since S are the IDs N = {W(S), ...}. So
//!   (a) and (b) hold when no ID in N lies in the request's phase/owner bucket
//!   and either equals the request or is a full orthant. `certify` scans N,
//!   and falls back when N exceeds `CERTIFY_LAG_LIMIT`. The bucket existed at
//!   S and buckets are never removed.
//!   - Orthant: at T the bucket's orthant is still o, whose rank still
//!     contains the request's rank, so the orthant return fires.
//!   - Containment: at S the orthant test failed, and by (b) it still fails.
//!     `revalidate` then returns f with the snapshot's forward charge exactly
//!     when its overflow guard passes (checked verbatim) and f is still live.
//!     f is live iff its summary slot is not released: retirement is the only
//!     index removal, and it releases the slot in the same step. Test and
//!     debug builds check this against `is_live`. The semantic flag is a
//!     function of the immutable domains of f and the request, so the helper
//!     computes it.
//!
//! Every precondition is evaluated before any mutation, including the checked
//! increments of the summary-build and semantic-hit counters. A fallback
//! therefore runs the unchanged slow path on the untouched state. That path
//! rebuilds what the helper did not ship: the transport domain (the compact
//! encoding is exact) and, for a verdict, the snapshot query (`try_new` is a
//! pure function of the domain and succeeded on the helper). The helper ships
//! the query and the prepared reverse set only for a snapshot miss.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Largest reverse set a helper may prepare; beyond it the commit retires on
/// the serial path. Bounds helper memory per in-flight proposal.
pub(in super::super) const PREPARED_RETIRE_LIMIT: usize = 65_536;

/// Most admissions since the snapshot that `certify` scans; beyond it the
/// verdict is committed on the slow path. A batch admits about 2 on average.
const CERTIFY_LAG_LIMIT: usize = 64;

/// Bounded-batch worker result, inseparably bound to its exact proposed domain.
/// This is lookup evidence, not admission or completion of pending work.
pub(in super::super) struct PreparedAdmission<const N: usize> {
    identity: u64,
    source: Source<N>,
    evidence: Evidence<N>,
    work: SpeculativeWork,
}

/// The proposed domain, in the form the ordered commit needs.
enum Source<const N: usize> {
    /// The compact image and exact-index digest, computed on the helper so the
    /// ordered commit does not hash it again. The helper dropped the transport
    /// form; a slow path rebuilds it with `expand` (the encoding is exact).
    Compact(CompactDomain<N>, Digest),
    /// Outside the compact range: the ordered commit refuses it.
    Transport(Domain<N>),
}

/// What the snapshot lookup found. Only a miss ships its query and reverse
/// set; see the module notes for when a verdict is final.
enum Evidence<const N: usize> {
    /// No usable speculation: finite-cap lane, cancelled lookup, or a summary
    /// the ordered commit refuses.
    None,
    /// Confirmed exact-index hit.
    Exact(usize),
    /// The bucket's full orthant contained the request's rank.
    Orthant { id: usize, watermark: usize },
    /// Minimum live containing ID, found after `checks` forward comparisons;
    /// `semantic` when the raw domain does not contain the request.
    Contained {
        id: usize,
        watermark: usize,
        checks: usize,
        semantic: bool,
    },
    Miss(Box<PreparedLookup<N>>),
}

/// The first failed precondition of a verdict that took the slow path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fallback {
    /// More than `CERTIFY_LAG_LIMIT` admissions since the snapshot.
    Lag,
    /// An admission since the snapshot equals the request.
    Equal,
    /// A same-bucket full orthant was admitted since the snapshot.
    Orthant,
    /// A checked counter or the revalidation overflow guard has no headroom.
    Headroom,
    /// The snapshot winner was retired since the snapshot.
    Retired,
}

impl SessionCounters {
    fn certify_fallback(&mut self, fallback: Fallback) {
        let counter = match fallback {
            Fallback::Lag => &mut self.certify_fallback_lag,
            Fallback::Equal => &mut self.certify_fallback_equal,
            Fallback::Orthant => &mut self.certify_fallback_orthant,
            Fallback::Headroom => &mut self.certify_fallback_headroom,
            Fallback::Retired => &mut self.certify_fallback_retired,
        };
        *counter = counter.saturating_add(1);
    }
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
        match &self.evidence {
            Evidence::Miss(lookup) => lookup.retire.as_ref().map(Vec::len),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn has_lookup(&self) -> bool {
        !matches!(self.evidence, Evidence::None)
    }

    /// The kind of snapshot evidence, for test setup assertions.
    #[cfg(test)]
    pub(super) fn verdict(&self) -> &'static str {
        match self.evidence {
            Evidence::None => "none",
            Evidence::Exact(_) => "exact",
            Evidence::Orthant { .. } => "orthant",
            Evidence::Contained { .. } => "contained",
            Evidence::Miss(_) => "miss",
        }
    }
}

/// Snapshot lookup as the slow path consumes it: shipped by the helper for a
/// miss, rebuilt at commit for a verdict that fell back.
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
        let Ok(compact) = CompactDomain::try_from_domain(&domain) else {
            // An unrepresentable domain is refused by the ordered commit itself.
            return PreparedAdmission {
                identity: self.identity,
                source: Source::Transport(domain),
                evidence: Evidence::None,
                work,
            };
        };
        let key = self.exact.key(&compact);
        let evidence = self.prepare_evidence(&domain, compact, key, &mut is_cancelled, &mut work);
        // Freed here, on the helper; only a slow path rebuilds it.
        drop(domain);
        PreparedAdmission {
            identity: self.identity,
            source: Source::Compact(compact, key),
            evidence,
            work,
        }
    }

    fn prepare_evidence(
        &self,
        domain: &Domain<N>,
        compact: CompactDomain<N>,
        key: Digest,
        is_cancelled: &mut impl FnMut() -> bool,
        work: &mut SpeculativeWork,
    ) -> Evidence<N> {
        if self.max_checks.is_some() || is_cancelled() {
            return Evidence::None;
        }
        if let Ok(id) = self.exact.get(key, &compact, &self.domains) {
            return Evidence::Exact(id);
        }
        // The ordered commit builds this summary before its orthant test, so
        // a refused summary is never an orthant or containment verdict.
        let Ok(core) = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        ) else {
            return Evidence::None;
        };
        let watermark = self.domains.len();
        let bucket = self.by_owner.get(&(domain.phase, domain.owner));
        if let Some(id) = bucket
            .and_then(|bucket| bucket.orthant)
            .filter(|&id| rank_contains(self.domains[id].rank(), domain.rank))
        {
            // Commit must still reproduce the ordinary summary preflight
            // and fresh exact/orthant priority, without a general scan.
            return if is_cancelled() {
                Evidence::None
            } else {
                Evidence::Orthant { id, watermark }
            };
        }
        let query = Query::new(core);
        let prefilter = self.prefilter;
        let stored = self.stored();
        let signature = Signature::of(&query.core);
        let coordinates = Coordinates::of(&query.core);
        let bucket_absent = bucket.is_none();
        let (found, retire) = if let Some(bucket) = bucket {
            let checkpoint = || {
                if is_cancelled() {
                    Err("cancelled speculative lookup")
                } else {
                    Ok(())
                }
            };
            let Ok(found) =
                bucket
                    .indexed
                    .find_controlled(signature, coordinates, 0, checkpoint, |id| {
                        work.checks = work
                            .checks
                            .checked_add(1)
                            .ok_or("speculative check overflow")?;
                        let rejected = prefilter.rejects(self.bits[id], query.word);
                        work.forward_bit_rejections = work
                            .forward_bit_rejections
                            .saturating_add(usize::from(rejected));
                        Ok(!rejected && stored.contains(id, &query))
                    })
            else {
                return Evidence::None;
            };
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
                            let rejected = prefilter.rejects(query.word, self.bits[id]);
                            work.reverse_bit_rejections = work
                                .reverse_bit_rejections
                                .saturating_add(usize::from(rejected));
                            Ok(!rejected && stored.contained_by(id, &query))
                        },
                    )
                    .ok()
            } else {
                None
            };
            (found, retire)
        } else {
            // No candidate of this phase/owner existed at the snapshot.
            (None, Some(Vec::new()))
        };
        if is_cancelled() {
            return Evidence::None;
        }
        match found {
            Some(id) => Evidence::Contained {
                id,
                watermark,
                checks: work.checks,
                semantic: !self.domains[id].contains(&compact),
            },
            None => Evidence::Miss(Box::new(PreparedLookup {
                query,
                watermark,
                found,
                checks: work.checks,
                retire,
                bucket_absent,
            })),
        }
    }

    /// Commit in the original proposal order. A stale preparation can avoid
    /// old comparisons, never avoid fresh exact/orthant checks, reverse
    /// maintenance, limits, reservations or pending-work publication.
    pub fn admit_prepared(
        &mut self,
        prepared: PreparedAdmission<N>,
    ) -> Result<(usize, bool), &'static str> {
        let PreparedAdmission {
            identity,
            source,
            evidence,
            work: _,
        } = prepared;
        let (compact, key) = match source {
            Source::Compact(compact, key) => (compact, key),
            Source::Transport(domain) => return self.admit_with_lookup(domain, None, None),
        };
        if identity != self.identity {
            // Neither the IDs nor the digest (a test key function) of another
            // queue apply here.
            return self.admit_with_lookup(compact.expand(), None, None);
        }
        if let Some(admitted) = self.certify(&compact, &evidence) {
            return admitted;
        }
        let domain = compact.expand();
        let lookup = if self.max_checks.is_none() {
            evidence
                .into_lookup(&domain)
                .filter(|lookup| lookup.watermark <= self.domains.len())
        } else {
            None
        };
        self.admit_with_lookup(domain, lookup, Some((compact, key)))
    }

    /// The early return that `admit_with_lookup` provably takes for this
    /// snapshot verdict (module notes), applied with its increments in its
    /// order. None, with the queue unchanged except fallback telemetry, when
    /// a precondition fails or the evidence is not a verdict.
    fn certify(
        &mut self,
        compact: &CompactDomain<N>,
        evidence: &Evidence<N>,
    ) -> Option<Result<(usize, bool), &'static str>> {
        #[cfg(test)]
        if !self.certified_verdicts_enabled || super::positive_reuse_trace::enabled() {
            return None;
        }
        if self.max_checks.is_some() {
            return None; // Never a verdict: the lane is fixed per queue.
        }
        let (id, watermark) = match *evidence {
            Evidence::Exact(id) => {
                #[cfg(any(test, debug_assertions))]
                self.assert_verdict_agrees(compact, evidence);
                self.exact_hits += 1;
                self.deduplicated += 1;
                self.session.certified_exact = self.session.certified_exact.saturating_add(1);
                return Some(Ok((id, false)));
            }
            Evidence::Orthant { id, watermark } | Evidence::Contained { id, watermark, .. } => {
                (id, watermark)
            }
            Evidence::None | Evidence::Miss(_) => return None,
        };
        let verdict = self.unchanged_since(compact, watermark).and_then(|()| {
            let builds = self
                .containment_summary_builds
                .checked_add(1)
                .ok_or(Fallback::Headroom)?;
            let Evidence::Contained {
                checks, semantic, ..
            } = *evidence
            else {
                return Ok((builds, None));
            };
            // `revalidate`'s overflow guard, verbatim.
            self.containment_checks
                .checked_add(checks)
                .and_then(|total| total.checked_add(self.summaries.ids().checked_mul(2)?))
                .ok_or(Fallback::Headroom)?;
            let semantic_hits = if semantic {
                self.containment_semantic_hits
                    .checked_add(1)
                    .ok_or(Fallback::Headroom)?
            } else {
                self.containment_semantic_hits
            };
            if self.summaries.is_released(id) {
                return Err(Fallback::Retired);
            }
            Ok((builds, Some((checks, semantic_hits))))
        });
        let (builds, contained) = match verdict {
            Ok(verdict) => verdict,
            Err(fallback) => {
                self.session.certify_fallback(fallback);
                return None;
            }
        };
        #[cfg(any(test, debug_assertions))]
        self.assert_verdict_agrees(compact, evidence);
        // The slow path's increments, in its order.
        self.containment_summary_builds = builds;
        if let Some((checks, semantic_hits)) = contained {
            self.containment_checks += checks; // bounded by the guard above
            self.containment_semantic_hits = semantic_hits;
            self.session.certified_contained = self.session.certified_contained.saturating_add(1);
        } else {
            self.orthant_hits += 1;
            self.session.certified_orthant = self.session.certified_orthant.saturating_add(1);
        }
        self.deduplicated += 1;
        Some(Ok((id, false)))
    }

    /// Preconditions (a) and (b) of the module notes: few admissions since
    /// the snapshot, and none of them in the request's bucket equal to it or
    /// a full orthant.
    pub(super) fn unchanged_since(
        &self,
        compact: &CompactDomain<N>,
        watermark: usize,
    ) -> Result<(), Fallback> {
        let since = self.domains.get(watermark..).ok_or(Fallback::Lag)?;
        if since.len() > CERTIFY_LAG_LIMIT {
            return Err(Fallback::Lag);
        }
        for admitted in since.iter().filter(|admitted| admitted.same_bucket(compact)) {
            if admitted == compact {
                return Err(Fallback::Equal);
            }
            if admitted.is_full_orthant() {
                return Err(Fallback::Orthant);
            }
        }
        Ok(())
    }

    /// Test and debug builds: the certified verdict is the one the serial
    /// lookup order derives from the current state; in particular a winner
    /// whose summary slot is not released is still live in the index.
    #[cfg(any(test, debug_assertions))]
    fn assert_verdict_agrees(&self, compact: &CompactDomain<N>, evidence: &Evidence<N>) {
        let exact = self.exact.get(self.exact.key(compact), compact, &self.domains);
        if let Evidence::Exact(id) = *evidence {
            assert_eq!(exact, Ok(id), "certified exact verdict");
            return;
        }
        assert!(exact.is_err(), "certified verdict of an exact duplicate");
        let bucket = &self.by_owner[&(compact.phase(), compact.owner())];
        let orthant = bucket
            .orthant
            .filter(|&orthant| rank_contains(self.domains[orthant].rank(), compact.rank()));
        match *evidence {
            Evidence::Orthant { id, .. } => assert_eq!(orthant, Some(id), "certified orthant"),
            Evidence::Contained { id, semantic, .. } => {
                assert_eq!(orthant, None, "certified winner behind an orthant");
                assert!(
                    bucket.indexed.is_live(self.stored().signature(id), id),
                    "unreleased summary slot of a retired winner"
                );
                assert_eq!(semantic, !self.domains[id].contains(compact));
            }
            Evidence::None | Evidence::Exact(_) | Evidence::Miss(_) => {
                unreachable!("not a verdict")
            }
        }
    }
}

impl<const N: usize> Evidence<N> {
    /// The prepared lookup that the slow path consumes. A verdict rebuilds
    /// the snapshot query from the domain exactly as the helper built it.
    fn into_lookup(self, domain: &Domain<N>) -> Option<PreparedLookup<N>> {
        let (found, watermark, checks) = match self {
            Self::None | Self::Exact(_) => return None,
            Self::Miss(lookup) => return Some(*lookup),
            Self::Orthant { watermark, .. } => (None, watermark, 0),
            Self::Contained {
                id,
                watermark,
                checks,
                ..
            } => (Some(id), watermark, checks),
        };
        let core = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )
        .ok()?;
        Some(PreparedLookup {
            query: Query::new(core),
            watermark,
            found,
            checks,
            retire: None,
            bucket_absent: false,
        })
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
        stored: Stored<'_, N>,
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
        // The bound counts every admitted ID, released summaries included.
        previous_checks
            .checked_add(self.checks)?
            .checked_add(stored.summaries.ids().checked_mul(2)?)?;
        if let Some(id) = self.found {
            // Snapshot misses before this minimum remain misses. New IDs are
            // greater, and other retirements cannot introduce an earlier hit.
            // A winner retired since the snapshot released its summary slot,
            // which a later admission may already reuse: never read it. It
            // is exactly the `is_live == false` fallback to the serial scan.
            if stored.summaries.is_released(id) {
                return None;
            }
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
        let found = index
            .find_from(
                Signature::of(&query.core),
                Coordinates::of(&query.core),
                self.watermark,
                |id| {
                    checks += 1; // bounded above before this scan
                    let rejected = prefilter.rejects(bits[id], query.word);
                    session.forward(rejected);
                    Ok(!rejected && stored.contains(id, query))
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
            trivial: self.bucket_absent,
        })
    }
}
