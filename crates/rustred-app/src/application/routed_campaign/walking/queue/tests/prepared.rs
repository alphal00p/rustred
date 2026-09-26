use super::*;
use std::sync::atomic::AtomicBool;

fn box_domain(lower: [u64; 2], upper: [u64; 2]) -> Domain<2> {
    Domain {
        lower: lower.to_vec(),
        upper: upper.into_iter().map(Some).collect(),
        ..domain(Some(10))
    }
}

pub(super) fn same_state<const N: usize>(serial: &Queue<N>, prepared: &Queue<N>) {
    assert_eq!(serial.domains, prepared.domains);
    assert_eq!(serial.summaries, prepared.summaries);
    assert_eq!(serial.bits, prepared.bits);
    assert_eq!(serial.exact, prepared.exact);
    assert_eq!(serial.by_owner.len(), prepared.by_owner.len());
    for (key, bucket) in &serial.by_owner {
        let other = &prepared.by_owner[key];
        assert_eq!(bucket.candidate_ids(), other.candidate_ids());
        // Physical layout too: the prepared reverse pass must retain, pin and
        // remove exactly what the serial pass does, in the same order.
        assert_eq!(bucket.indexed.layout(), other.indexed.layout());
        assert_eq!(bucket.orthant, other.orthant);
    }
    assert_eq!(serial.next, prepared.next);
    assert_eq!(serial.deduplicated, prepared.deduplicated);
    assert_eq!(serial.exact_hits, prepared.exact_hits);
    assert_eq!(serial.orthant_hits, prepared.orthant_hits);
    assert_eq!(serial.max_finite_rank, prepared.max_finite_rank);
    assert_eq!(
        serial.unbounded_rank_domains,
        prepared.unbounded_rank_domains
    );
    assert_eq!(
        serial.containment_summary_builds,
        prepared.containment_summary_builds
    );
    assert_eq!(
        serial.containment_retired_candidates,
        prepared.containment_retired_candidates
    );
    assert_eq!(
        serial.containment_semantic_hits,
        prepared.containment_semantic_hits
    );
    assert_eq!(
        serial.containment_semantic_retirements,
        prepared.containment_semantic_retirements
    );
    assert_eq!(
        serial.containment_maintenance_checks,
        prepared.containment_maintenance_checks
    );
    // Completed speculative forward work can differ from a fresh serial scan.
    // This telemetry is deliberately not a semantic state-equality condition.
}

pub(super) fn parallel_prepare<const N: usize>(
    queue: &Queue<N>,
    batch: &[Domain<N>],
    workers: usize,
) -> Vec<PreparedAdmission<N>> {
    let cancellation = &AtomicBool::new(false);
    if workers == 1 {
        return batch
            .iter()
            .cloned()
            .map(|item| queue.prepare_admission(item, cancellation))
            .collect();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = batch
            .chunks(batch.len().div_ceil(workers))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .cloned()
                        .map(|item| queue.prepare_admission(item, cancellation))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    })
}

#[test]
fn parallel_preparation_matches_complete_proposal_streams_in_serial_commit_order() {
    let original = super::aggregate::complete_proposals();
    for (order, workers, batch_size) in [(0, 1, 1), (1, 3, 17), (2, 6, 256)] {
        let mut stream = original.clone();
        if order == 1 {
            stream.reverse();
        } else if order == 2 {
            let mut state = 43_u64;
            for i in (1..stream.len()).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                stream.swap(i, state as usize % (i + 1));
            }
        }
        let mut serial = Queue::new(stream.len(), None);
        let mut prepared = Queue::new(stream.len(), None);
        let mut linear = super::linear_semantic::LinearSemantic::new(stream.len());
        for batch in stream.chunks(batch_size) {
            let tokens = parallel_prepare(&prepared, batch, workers);
            for (item, token) in batch.iter().zip(tokens) {
                let expected = serial.admit(item.clone());
                assert_eq!(expected, linear.admit(item.clone()));
                assert_eq!(prepared.admit_prepared(token), expected);
                same_state(&serial, &prepared);
                linear.assert_same_state(&prepared);
            }
        }
        println!(
            "prepared_complete_stream order={order} workers={workers} batch={batch_size} proposals={} admitted={} serial_checks={} prepared_checks={}",
            stream.len(),
            serial.domains.len(),
            serial.containment_checks,
            prepared.containment_checks
        );
    }
}

#[test]
fn retired_snapshot_winner_falls_back_to_current_minimum_not_its_replacement() {
    let mut queue = Queue::new(20, None);
    queue.admit(box_domain([0, 0], [2, 3])).unwrap();
    queue.admit(box_domain([1, 0], [5, 2])).unwrap();
    let request = box_domain([1, 1], [1, 1]);
    let token = queue.prepare_admission(request, &AtomicBool::new(false));
    assert_eq!(queue.admit(box_domain([0, 0], [3, 4])), Ok((2, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].candidate_ids(),
        [1, 2]
    );
    assert_eq!(queue.admit_prepared(token), Ok((1, false)));
    assert_eq!(queue.domains.len(), 3);
    assert_eq!(queue.next, 0);
}

#[test]
fn prepared_miss_sees_new_candidate_and_fresh_exact_orthant_priority() {
    let mut queue = Queue::new(20, None);
    queue.admit(box_domain([0, 0], [2, 3])).unwrap();
    let request = box_domain([5, 1], [5, 1]);
    let token = queue.prepare_admission(request.clone(), &AtomicBool::new(false));
    assert_eq!(queue.admit(box_domain([4, 0], [7, 3])), Ok((1, true)));
    assert_eq!(queue.admit_prepared(token), Ok((1, false)));

    let fresh = box_domain([10, 1], [10, 1]);
    let exact = queue.prepare_admission(fresh.clone(), &AtomicBool::new(false));
    assert_eq!(queue.admit(fresh), Ok((2, true)));
    assert_eq!(queue.admit_prepared(exact), Ok((2, false)));
    assert_eq!(queue.exact_hits, 1);

    let before_orthant = queue.prepare_admission(request, &AtomicBool::new(false));
    assert_eq!(queue.admit(domain(Some(10))), Ok((3, true)));
    assert_eq!(queue.admit_prepared(before_orthant), Ok((3, false)));
    assert_eq!(queue.orthant_hits, 1);
}

#[test]
fn prepared_filtered_miss_sees_append_that_widens_the_same_nonfull_block() {
    let diagonal = |lo, hi| Domain {
        phase: Phase::Apply,
        owner: [true; 2],
        lower: vec![lo, 100 - hi],
        upper: vec![Some(hi), Some(100 - lo)],
        rank: None,
        powers: DomainPowerBounds {
            max_positive_power: Some(102),
            min_power_difference: Some(102),
            max_power_difference: Some(102),
        },
    };
    let mut queue = Queue::new(20, None);
    assert_eq!(queue.admit(diagonal(0, 0)), Ok((0, true)));
    let request = diagonal(70, 70);
    let token = queue.prepare_admission(request, &AtomicBool::new(false));
    assert_eq!(
        token.speculative_checks(),
        0,
        "the original coordinate block rejects"
    );
    assert_eq!(queue.admit(diagonal(60, 80)), Ok((1, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true; 2])].indexed.groups(),
        1
    );
    // find_from must revisit the widened existing block while skipping its
    // disproved ID0 prefix; the original request is not a fresh exact-key hit.
    assert_eq!(queue.admit_prepared(token), Ok((1, false)));
    assert_eq!(queue.exact_hits, 0);
    assert_eq!(queue.domains.len(), 2);
}

#[test]
fn foreign_queue_tokens_and_cancelled_preparation_fall_back_without_mutation() {
    let mut first = Queue::new(10, None);
    first.admit(box_domain([0, 0], [8, 4])).unwrap();
    let request = box_domain([2, 2], [2, 2]);
    let token = first.prepare_admission(request.clone(), &AtomicBool::new(false));
    let mut second = Queue::new(10, None);
    second.admit(box_domain([6, 0], [8, 4])).unwrap();
    assert_eq!(second.admit_prepared(token), Ok((1, true)));
    let before = second.containment_checks;
    let cancelled = second.prepare_admission(request, &AtomicBool::new(true));
    assert_eq!(second.containment_checks, before);
    assert_eq!(second.admit_prepared(cancelled), Ok((1, false)));
}

#[test]
fn cancellation_at_each_lookup_checkpoint_preserves_attempted_work_and_ordered_semantics() {
    let mut a = box_domain([0, 1], [3, 3]);
    a.owner = [true; 2];
    let mut b = a.clone();
    b.lower = vec![1, 0];
    let mut request = b.clone();
    request.lower = vec![2, 0];
    request.upper = vec![Some(2), Some(0)];
    for allowed in 0_usize..=4 {
        let mut serial = Queue::new(20, None);
        let mut prepared = Queue::new(20, None);
        for item in [a.clone(), b.clone()] {
            serial.admit(item.clone()).unwrap();
            prepared.admit(item).unwrap();
        }
        let mut checkpoints = 0;
        let token = prepared.prepare_admission_check(request.clone(), || {
            checkpoints += 1;
            checkpoints > allowed
        });
        // Checkpoints now include uncharged group/block rejection boundaries.
        // This two-candidate block performs its two native comparisons only
        // after the entry, group and block checkpoints have all passed.
        assert_eq!(token.speculative_checks(), if allowed >= 3 { 2 } else { 0 });
        same_state(&serial, &prepared); // Even mid-scan cancellation is read-only.
        assert_eq!(serial.containment_checks, prepared.containment_checks);
        assert_eq!(
            prepared.admit_prepared(token),
            serial.admit(request.clone())
        );
        same_state(&serial, &prepared);
    }
    let queue = Queue::<2>::new(20, None);
    let token =
        queue.prepare_admission_with_stop(request, &AtomicBool::new(false), &AtomicBool::new(true));
    assert_eq!(token.speculative_checks(), 0);
}

#[test]
fn prepared_failures_keep_ordered_error_prefix_and_finite_cap_behavior() {
    for cap in [None, Some(0), Some(1), Some(10)] {
        let mut serial = Queue::new(2, cap);
        let mut prepared = Queue::new(2, cap);
        let mut invalid = box_domain([0, 0], [3, 3]);
        invalid.powers.min_power_difference = Some(4);
        invalid.powers.max_power_difference = Some(3);
        let stream = [
            box_domain([0, 0], [2, 3]),
            box_domain([5, 0], [7, 3]),
            box_domain([9, 0], [11, 3]),
            invalid,
            box_domain([0, 0], [2, 3]),
            box_domain([1, 1], [1, 1]),
        ];
        let tokens = parallel_prepare(&prepared, &stream, 3);
        for (item, token) in stream.into_iter().zip(tokens) {
            assert_eq!(prepared.admit_prepared(token), serial.admit(item));
            same_state(&serial, &prepared);
            if cap.is_some() {
                assert_eq!(serial.containment_checks, prepared.containment_checks);
            }
        }
    }
}

#[test]
fn empty_infinite_and_near_counter_exhaustion_tokens_preserve_serial_results() {
    let mut serial = Queue::new(30, None);
    let mut prepared = Queue::new(30, None);
    let mut empty = domain(None);
    empty.powers.max_positive_power = Some(0);
    let mut finite_max = domain(Some(u32::MAX));
    finite_max.upper[0] = Some(u64::MAX);
    let mut infinite = domain(None);
    infinite.lower[0] = u64::MAX;
    let stream = [empty, finite_max, infinite, domain(None), domain(Some(5))];
    let tokens = parallel_prepare(&prepared, &stream, 3);
    for (item, token) in stream.into_iter().zip(tokens) {
        assert_eq!(prepared.admit_prepared(token), serial.admit(item));
        same_state(&serial, &prepared);
    }
    for value in [usize::MAX - 1, usize::MAX] {
        serial.containment_summary_builds = value;
        prepared.containment_summary_builds = value;
        let mut request = domain(Some(4));
        request.lower[0] = 5;
        let token = prepared.prepare_admission(request.clone(), &AtomicBool::new(false));
        assert_eq!(prepared.admit_prepared(token), serial.admit(request));
        same_state(&serial, &prepared);
    }
}

#[test]
fn stale_lookup_counter_overflow_falls_back_for_both_forward_and_reverse_work() {
    // The old first group contains A then B. A misses on its second lower
    // coordinate, although aggregate signatures cannot reject it.
    let mut a = box_domain([0, 1], [3, 3]);
    a.owner = [true; 2];
    let mut b = a.clone();
    b.lower = vec![1, 0];
    let mut wider_a = a.clone();
    wider_a.upper[0] = Some(4);
    let mut hit = b.clone();
    hit.lower = vec![2, 0];
    hit.upper = vec![Some(2), Some(0)];
    let mut miss = a.clone();
    miss.lower = vec![0, 0];
    miss.upper = vec![Some(4), Some(3)];
    for request in [hit, miss] {
        for remaining in 0..8 {
            let mut serial = Queue::new(20, None);
            let mut prepared = Queue::new(20, None);
            for item in [a.clone(), b.clone()] {
                serial.admit(item.clone()).unwrap();
                prepared.admit(item).unwrap();
            }
            let token = prepared.prepare_admission(request.clone(), &AtomicBool::new(false));
            serial.admit(wider_a.clone()).unwrap();
            prepared.admit(wider_a.clone()).unwrap();
            serial.containment_checks = usize::MAX - remaining;
            prepared.containment_checks = usize::MAX - remaining;
            assert_eq!(
                prepared.admit_prepared(token),
                serial.admit(request.clone())
            );
            same_state(&serial, &prepared);
            assert_eq!(serial.containment_checks, prepared.containment_checks);
        }
    }
}

/// Overlapping boxes with in-batch containment chains: every third proposal
/// contains its two predecessors, wider bands retire whole runs, and repeated
/// exact keys and unrelated owners interleave.
fn overlapping_stream() -> Vec<Domain<2>> {
    let mut stream = Vec::new();
    for i in 0..60_u64 {
        let base = i * 3;
        stream.push(box_domain([base, 1], [base + 1, 2]));
        stream.push(box_domain([base, 0], [base + 2, 2]));
        stream.push(box_domain([base, 0], [base + 3, 3])); // contains both above
        if i % 4 == 3 {
            stream.push(box_domain([base - 9, 0], [base + 3, 3])); // retires a run
        }
        if i % 5 == 0 {
            stream.push(box_domain([base, 1], [base + 1, 2])); // exact repeat
            let mut other = box_domain([base, 0], [base + 3, 3]);
            other.owner = [false, true];
            stream.push(other);
        }
    }
    stream.push(box_domain([0, 0], [200, 3])); // retires nearly everything
    stream.push(box_domain([5, 0], [6, 1])); // reuse under the wide band
    stream
}

fn ledger_queue(policy: usize, len: usize) -> Queue<2> {
    let mut queue = Queue::new(len, None);
    let lookahead = std::num::NonZeroUsize::new(2).unwrap();
    queue.delegation = match policy {
        0 => None,
        1 => Some(Ledger::new(lookahead, len).unwrap()),
        _ => Some(Ledger::new_ready(lookahead, len).unwrap()),
    };
    queue
}

fn ledger_summary(queue: &Queue<2>) -> Option<(usize, super::super::super::delegation::Summary)> {
    queue
        .delegation
        .as_ref()
        .map(|ledger| (ledger.transfer_count(), ledger.resolve().unwrap().summary))
}

#[test]
fn prepared_retirement_matches_serial_retire_set_layout_and_transfers() {
    for policy in 0..3 {
        for (workers, batch_size) in [(1, 1), (3, 7), (6, 64)] {
            let stream = overlapping_stream();
            let mut serial = ledger_queue(policy, stream.len());
            let mut prepared = ledger_queue(policy, stream.len());
            let mut applied_any = false;
            for batch in stream.chunks(batch_size) {
                let tokens = parallel_prepare(&prepared, batch, workers);
                for (item, token) in batch.iter().zip(tokens) {
                    applied_any |= token.prepared_retire_len().is_some_and(|len| len > 0);
                    let expected = serial.admit(item.clone());
                    assert_eq!(prepared.admit_prepared(token), expected);
                    same_state(&serial, &prepared);
                    assert_eq!(serial.containment_checks, prepared.containment_checks);
                    assert_eq!(
                        serial.containment_maintenance_checks,
                        prepared.containment_maintenance_checks
                    );
                    assert_eq!(ledger_summary(&serial), ledger_summary(&prepared));
                }
            }
            assert!(
                applied_any,
                "policy {policy}: no prepared reverse set was ever non-empty"
            );
            assert!(prepared.session.prepared_retirements_applied > 0);
            assert_eq!(prepared.session.prepared_retire_fallbacks, 0);
            // The second-owner proposals are 16 positions apart: only the
            // 64-record batches prepare one before an in-batch commit has
            // created its bucket, and that empty set is counted as trivial.
            if batch_size == 64 {
                assert!(prepared.session.prepared_retirements_trivial > 0);
            } else {
                assert_eq!(prepared.session.prepared_retirements_trivial, 0);
            }
            assert_eq!(serial.session.prepared_retirements_applied, 0);
            assert_eq!(serial.session.prepared_retirements_trivial, 0);
            assert!(serial.containment_retired_candidates > 10);
            if policy != 0 {
                let (transfers, summary) = ledger_summary(&serial).unwrap();
                assert!(transfers > 0, "policy {policy}: no transfer happened");
                assert!(
                    transfers < serial.containment_retired_candidates,
                    "policy {policy}: reserved olds must stay native: {summary:?}"
                );
            }
            println!(
                "prepared_retirement policy={policy} workers={workers} batch={batch_size} retired={} session={:?}",
                serial.containment_retired_candidates, prepared.session
            );
        }
    }
}

#[test]
fn prepared_retirement_scans_in_batch_admissions_above_the_watermark() {
    let mut queue = Queue::new(20, None);
    assert_eq!(queue.admit(box_domain([1, 1], [2, 2])), Ok((0, true)));
    assert_eq!(queue.admit(box_domain([50, 0], [60, 3])), Ok((1, true)));
    let request = box_domain([0, 0], [9, 9]);
    let token = queue.prepare_admission(request.clone(), &AtomicBool::new(false));
    assert_eq!(token.prepared_retire_len(), Some(1)); // only ID 0 at the snapshot
    assert_eq!(token.speculative_work().reverse_checks, 1);
    // An in-batch admission above the watermark, also contained by the request.
    assert_eq!(queue.admit(box_domain([3, 3], [4, 4])), Ok((2, true)));
    let before = queue.session;
    assert_eq!(queue.admit_prepared(token), Ok((3, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].candidate_ids(),
        [1, 3]
    );
    assert_eq!(queue.containment_retired_candidates, 2);
    assert_eq!(
        queue.session.prepared_retirements_applied,
        before.prepared_retirements_applied + 1
    );
    // Only the new ID 2 needed a commit-time comparison; ID 0 came from the set.
    assert_eq!(
        queue.session.reverse_callbacks,
        before.reverse_callbacks + 1
    );
    let mut serial = Queue::new(20, None);
    for item in [
        box_domain([1, 1], [2, 2]),
        box_domain([50, 0], [60, 3]),
        box_domain([3, 3], [4, 4]),
        request,
    ] {
        serial.admit(item).unwrap();
    }
    same_state(&serial, &queue);
    assert_eq!(serial.containment_checks, queue.containment_checks);
}

#[test]
fn empty_prepared_set_from_an_absent_bucket_counts_as_trivial_not_applied() {
    let mut queue = Queue::new(20, None);
    queue.admit(box_domain([0, 0], [2, 3])).unwrap(); // owner [true, false]
    let mut first = box_domain([10, 0], [12, 3]);
    first.owner = [false, true];
    let mut second = box_domain([20, 0], [22, 3]);
    second.owner = [false, true];
    let tokens = parallel_prepare(&queue, &[first.clone(), second.clone()], 1);
    for token in &tokens {
        assert_eq!(token.prepared_retire_len(), Some(0), "bucket absent");
        assert_eq!(token.speculative_work().reverse_checks, 0);
    }
    let [one, two] = <[_; 2]>::try_from(tokens).ok().unwrap();
    // A brand-new bucket has nothing to retire and counts nothing.
    assert_eq!(queue.admit_prepared(one), Ok((1, true)));
    let session = queue.session;
    assert_eq!(
        (
            session.prepared_retirements_applied,
            session.prepared_retirements_trivial,
            session.prepared_retire_fallbacks
        ),
        (0, 0, 0)
    );
    // The bucket now exists: the empty snapshot set is applied but decided
    // nothing, so it counts as trivial and the in-batch ID is compared here.
    assert_eq!(queue.admit_prepared(two), Ok((2, true)));
    let session = queue.session;
    assert_eq!(
        (
            session.prepared_retirements_applied,
            session.prepared_retirements_trivial,
            session.prepared_retire_fallbacks
        ),
        (0, 1, 0)
    );
    assert_eq!(session.reverse_callbacks, 1);
    let mut serial = Queue::new(20, None);
    for item in [box_domain([0, 0], [2, 3]), first, second] {
        serial.admit(item).unwrap();
    }
    same_state(&serial, &queue);
    assert_eq!(serial.containment_checks, queue.containment_checks);
}

#[test]
fn prepared_retirement_falls_back_for_retired_winners_and_near_counter_exhaustion() {
    // Retired snapshot winner: the fresh scan hits its wider replacement, so
    // nothing is retired and no prepared set is consulted.
    let mut queue = Queue::new(20, None);
    queue.admit(box_domain([0, 0], [2, 3])).unwrap();
    let token = queue.prepare_admission(box_domain([1, 1], [1, 1]), &AtomicBool::new(false));
    assert_eq!(token.prepared_retire_len(), None);
    assert_eq!(queue.admit(box_domain([0, 0], [3, 4])), Ok((1, true)));
    assert_eq!(queue.admit_prepared(token), Ok((1, false)));
    assert_eq!(queue.session.prepared_retirements_applied, 0);
    assert_eq!(queue.session.prepared_retire_fallbacks, 0);
    // Near counter exhaustion a prepared miss must retire on the serial path
    // (and count a fallback) with results identical to a serial queue.
    for remaining in 0..6 {
        let mut serial = Queue::new(20, None);
        let mut prepared = Queue::new(20, None);
        for item in [box_domain([1, 1], [2, 2]), box_domain([5, 0], [7, 3])] {
            serial.admit(item.clone()).unwrap();
            prepared.admit(item).unwrap();
        }
        let request = box_domain([0, 0], [3, 3]);
        let token = prepared.prepare_admission(request.clone(), &AtomicBool::new(false));
        assert_eq!(token.prepared_retire_len(), Some(1));
        serial.containment_checks = usize::MAX - remaining;
        prepared.containment_checks = usize::MAX - remaining;
        assert_eq!(prepared.admit_prepared(token), serial.admit(request));
        same_state(&serial, &prepared);
        assert_eq!(serial.containment_checks, prepared.containment_checks);
        if prepared.domains.len() == 3 {
            // Revalidation needs 2 x summaries.len() = 4 units of headroom;
            // below that the prepared set is dropped and the serial scan
            // retires (a counted fallback), above it the set is applied.
            let fell_back = remaining < 4;
            assert_eq!(
                prepared.session.prepared_retirements_applied,
                usize::from(!fell_back),
                "remaining {remaining}"
            );
            assert_eq!(
                prepared.session.prepared_retire_fallbacks,
                usize::from(fell_back),
                "remaining {remaining}"
            );
        }
    }
}

#[test]
fn cancelled_reverse_preparation_publishes_nothing() {
    let fixture = || {
        let mut queue = Queue::new(20, None);
        for item in [box_domain([1, 1], [2, 2]), box_domain([5, 0], [7, 3])] {
            queue.admit(item).unwrap();
        }
        queue
    };
    let serial = fixture();
    let prepared = fixture();
    let request = box_domain([0, 0], [3, 3]);
    let uncancelled = prepared.prepare_admission_check(request.clone(), || false);
    let mut total_checkpoints = 0;
    let _ = prepared.prepare_admission_check(request.clone(), || {
        total_checkpoints += 1;
        false
    });
    assert!(uncancelled.has_lookup() && uncancelled.prepared_retire_len() == Some(1));
    assert!(
        total_checkpoints >= 6,
        "entry, forward group/block, reverse group/block and final checkpoints"
    );
    let mut seen_reverse = false;
    // Cancel at every checkpoint, so some tokens are cut off inside the
    // reverse collection itself after a completed forward miss.
    for allowed in 1..total_checkpoints {
        let mut checkpoints = 0;
        let token = prepared.prepare_admission_check(request.clone(), || {
            checkpoints += 1;
            checkpoints > allowed
        });
        let work = token.speculative_work();
        seen_reverse |= work.reverse_checks > 0;
        assert!(!token.has_lookup(), "cancelled speculation is discarded");
        assert_eq!(token.prepared_retire_len(), None);
        same_state(&serial, &prepared);
        let mut serial_clone = fixture();
        let mut prepared_clone = fixture();
        let mut checkpoints = 0;
        let token = prepared_clone.prepare_admission_check(request.clone(), || {
            checkpoints += 1;
            checkpoints > allowed
        });
        assert_eq!(
            prepared_clone.admit_prepared(token),
            serial_clone.admit(request.clone())
        );
        same_state(&serial_clone, &prepared_clone);
        assert_eq!(prepared_clone.session.prepared_retirements_applied, 0);
        assert_eq!(prepared_clone.session.prepared_retire_fallbacks, 0);
    }
    assert!(
        seen_reverse,
        "some cancellation must land inside the reverse scan"
    );
}
