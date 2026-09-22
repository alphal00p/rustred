use super::linear_semantic::LinearSemantic;
use super::*;

/// Every proposal is retained, including duplicate keys and semantically
/// contained requests that will never become admitted domain descriptors.
pub(super) fn complete_proposals() -> Vec<Domain<2>> {
    let mut stream = Vec::new();
    let intervals = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (0, 2)];
    for (lo0, hi0) in intervals {
        for (lo1, hi1) in intervals {
            for owner in [[true, false], [false, true], [true; 2], [false; 2]] {
                for phase in [Phase::Apply, Phase::Route] {
                    for rank in [Some(0), Some(1), Some(3), None] {
                        for a in [None, Some(0), Some(2), Some(4)] {
                            for (dmin, dmax) in [
                                (None, None),
                                (Some(-1), None),
                                (Some(1), None),
                                (None, Some(0)),
                                (Some(0), Some(0)),
                            ] {
                                let d = Domain {
                                    phase,
                                    owner,
                                    lower: vec![lo0, lo1],
                                    upper: vec![Some(hi0), Some(hi1)],
                                    rank,
                                    powers: DomainPowerBounds {
                                        max_positive_power: a,
                                        min_power_difference: dmin,
                                        max_power_difference: dmax,
                                    },
                                };
                                stream.push(d.clone());
                                stream.push(d); // Repeated proposals are not prefiltered.
                            }
                        }
                    }
                }
            }
        }
    }
    stream
}

#[test]
fn aggregate_index_replays_complete_proposal_stream_with_identical_returned_ids() {
    let original = complete_proposals();
    assert_eq!(original.len(), 46_080);
    for order in 0..3 {
        let mut stream = original.clone();
        match order {
            1 => stream.reverse(),
            2 => {
                // Deterministic order perturbation, not a numerical probe.
                let mut state = 31_u64;
                for i in (1..stream.len()).rev() {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    stream.swap(i, state as usize % (i + 1));
                }
            }
            _ => {}
        }
        let mut queue = Queue::new(stream.len(), None);
        let mut linear = LinearSemantic::new(stream.len());
        let mut reused = 0;
        for (position, request) in stream.into_iter().enumerate() {
            let expected = linear.admit(request.clone());
            let actual = queue.admit(request);
            assert_eq!(actual, expected, "order={order}, proposal={position}");
            reused += usize::from(matches!(actual, Ok((_, false))));
            linear.assert_same_state(&queue);
        }
        assert!(reused > 1000);
        let group_visits: usize = queue
            .by_owner
            .values()
            .map(|bucket| bucket.indexed.work().groups_visited)
            .sum();
        let group_rejections: usize = queue
            .by_owner
            .values()
            .map(|bucket| bucket.indexed.work().groups_rejected)
            .sum();
        println!(
            "complete_proposal_replay order={order} proposals={} admitted={} reused={reused} linear_checks={} indexed_checks={} group_visits={group_visits} group_rejections={group_rejections}",
            original.len(),
            queue.domains.len(),
            linear.comparisons,
            queue.containment_checks
        );
        assert!(group_visits > 0);
        assert!(group_rejections > 0);
    }
}

#[test]
fn aggregate_index_preserves_admission_errors_and_later_reuse_in_complete_stream() {
    let mut queue = Queue::new(2, None);
    let mut linear = LinearSemantic::new(2);
    let mut a = domain(Some(4));
    a.lower[0] = 1;
    a.upper = vec![Some(2), Some(4)];
    let mut b = a.clone();
    b.lower[0] = 5;
    b.upper[0] = Some(6);
    let mut c = b.clone();
    c.lower[0] = 10;
    c.upper[0] = Some(12);
    let mut invalid = a.clone();
    invalid.powers.min_power_difference = Some(3);
    invalid.powers.max_power_difference = Some(2);
    let mut contained = a.clone();
    contained.rank = None; // Same actual rank through the finite box.
    let mut errors = 0;
    for request in [
        a.clone(),
        b.clone(),
        c.clone(),
        a.clone(),
        invalid,
        contained,
        c,
        b,
        a,
    ] {
        let expected = linear.admit(request.clone());
        let actual = queue.admit(request);
        assert_eq!(actual, expected);
        errors += usize::from(actual.is_err());
        linear.assert_same_state(&queue);
    }
    assert_eq!(errors, 3);
    assert_eq!(queue.next, 0);
}

#[test]
fn aggregate_index_matches_linear_semantics_for_infinities_empty_cases_and_ties() {
    let mut queue = Queue::new(1000, None);
    let mut linear = LinearSemantic::new(1000);
    let mut stream = Vec::new();
    for (lower, upper) in [
        ([0, 2], [Some(10), Some(4)]),
        ([3, 0], [Some(8), Some(6)]),
        ([4, 2], [Some(5), Some(3)]), // In both earlier incomparable boxes.
        ([0, 0], [Some(u64::MAX), None]),
        ([u64::MAX, 0], [None, None]),
        ([0, 0], [None, None]),
    ] {
        for rank in [Some(u32::MAX), None] {
            for phase in [Phase::Apply, Phase::Route] {
                let mut item = domain(rank);
                item.phase = phase;
                item.lower = lower.to_vec();
                item.upper = upper.to_vec();
                stream.push(item.clone());
                item.powers.max_positive_power = Some(0); // Empty active support.
                stream.push(item.clone());
                item.rank = Some(1); // Different raw description of an empty set.
                stream.push(item);
            }
        }
    }
    for request in stream {
        assert_eq!(queue.admit(request.clone()), linear.admit(request));
        linear.assert_same_state(&queue);
    }
}
