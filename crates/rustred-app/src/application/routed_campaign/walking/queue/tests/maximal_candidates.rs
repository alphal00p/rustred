use super::*;

fn bounded(rank: Option<u32>, lower: u64, upper: u64) -> Domain<2> {
    let mut value = domain(rank);
    value.lower[0] = lower;
    value.upper = vec![Some(upper), Some(4)];
    value
}

fn assert_maximal_index(queue: &Queue<2>) {
    assert_eq!(
        queue.containment_candidate_count(),
        queue
            .by_owner
            .values()
            .map(|bucket| bucket.candidate_ids().len())
            .sum::<usize>()
    );
    for (&(phase, owner), bucket) in &queue.by_owner {
        let ids = bucket.candidate_ids();
        assert!(ids.windows(2).all(|ids| ids[0] < ids[1]));
        for &left in &ids {
            for &right in &ids {
                if left != right {
                    assert!(!semantic_contains(
                        &queue.domains[left],
                        &queue.domains[right]
                    ));
                }
            }
        }
        for historic in queue
            .domains
            .iter()
            .filter(|d| d.phase == phase && d.owner == owner)
        {
            assert!(
                ids.iter()
                    .any(|&id| semantic_contains(&queue.domains[id], historic))
            );
        }
    }
}

#[test]
fn maximal_candidates_retire_only_index_entries_and_keep_exact_ids_and_pending_work() {
    let mut queue = Queue::new(8, None);
    assert_eq!(
        queue.containment_index_policy(),
        "maximal_candidates_semantic_unlimited"
    );
    let narrow = bounded(Some(11), 3, 9);
    let broad = bounded(Some(11), 0, 12);
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    assert_eq!(queue.admit(broad.clone()), Ok((1, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, narrow.owner)].candidate_ids(),
        vec![1]
    );
    assert_eq!(queue.domains.len(), 2);
    assert_eq!(queue.exact.len(), 2);
    assert_eq!(queue.containment_retired_candidates, 1);
    assert_eq!(queue.containment_candidate_count(), 1);
    assert_eq!(queue.next, 0); // Neither obligation has been processed.
    assert_eq!(queue.admit(narrow.clone()), Ok((0, false)));
    assert_eq!(queue.admit(bounded(Some(10), 5, 8)), Ok((1, false)));
    assert_eq!(queue.containment_maintenance_checks, 1);
    // The widening's forward comparison is excluded by its aggregate maximum;
    // exact reverse maintenance and the subsequent fresh-child check remain.
    assert_eq!(queue.containment_checks, 2);
    assert_eq!(queue.domains[0].as_ref(), &narrow);
    assert_eq!(queue.domains[1].as_ref(), &broad);
    assert_maximal_index(&queue);
}

#[test]
fn maximal_candidates_keep_high_rank_finite_boxes_beside_lower_rank_orthants() {
    let mut queue = Queue::new(8, None);
    assert_eq!(queue.admit(domain(Some(5))), Ok((0, true)));
    let mut high = bounded(Some(10), 2, 8);
    // This box must really contain R>5, not merely carry a redundant rank10 label.
    high.upper[1] = Some(10);
    assert_eq!(queue.admit(high.clone()), Ok((1, true)));
    assert_eq!(queue.admit(domain(Some(7))), Ok((2, true)));
    let bucket = &queue.by_owner[&(Phase::Apply, high.owner)];
    assert_eq!(bucket.candidate_ids(), vec![1, 2]);
    assert_eq!(bucket.orthant, Some(2));
    assert_eq!(queue.containment_retired_candidates, 1);
    assert_eq!(queue.containment_candidate_count(), 2);
    let mut high_child = bounded(Some(8), 3, 7);
    high_child.upper[1] = Some(8);
    assert_eq!(queue.admit(high_child), Ok((1, false)));
    assert_eq!(queue.admit(domain(Some(5))), Ok((0, false)));
    assert_maximal_index(&queue);
}

#[test]
fn maximal_candidates_distinguish_rank_and_coordinate_infinity() {
    let mut queue = Queue::new(8, None);
    let mut finite_rank = bounded(Some(u32::MAX), 0, u64::MAX);
    finite_rank.upper[1] = None;
    let mut unbounded_rank = bounded(None, 0, u64::MAX);
    unbounded_rank.upper[1] = None;
    assert_eq!(queue.admit(finite_rank.clone()), Ok((0, true)));
    assert_eq!(queue.admit(unbounded_rank.clone()), Ok((1, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, finite_rank.owner)].candidate_ids(),
        vec![1]
    );
    // A genuinely unbounded positive axis is not contained in u64::MAX.
    let mut infinite_axis = unbounded_rank.clone();
    infinite_axis.upper[0] = None;
    assert_eq!(queue.admit(infinite_axis), Ok((2, true)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, finite_rank.owner)].candidate_ids(),
        vec![2]
    );
    assert_eq!(queue.admit(finite_rank), Ok((0, false)));
    assert_eq!(queue.admit(unbounded_rank), Ok((1, false)));
    assert_eq!(queue.containment_retired_candidates, 2);
    assert_eq!(queue.containment_candidate_count(), 1);
    assert_eq!(queue.next, 0);
    assert_maximal_index(&queue);
}

#[test]
fn maximal_candidates_failed_admission_never_retires_the_existing_representative() {
    let mut queue = Queue::new(1, None);
    let narrow = bounded(Some(11), 3, 9);
    let broad = bounded(Some(11), 0, 12);
    queue.admit(narrow.clone()).unwrap();
    assert_eq!(
        queue.admit(broad.clone()),
        Err("scheduled domain allowance")
    );
    assert_eq!(
        queue.by_owner[&(Phase::Apply, narrow.owner)].candidate_ids(),
        vec![0]
    );
    assert_eq!(queue.containment_maintenance_checks, 0);
    assert_eq!(queue.containment_retired_candidates, 0);
    assert_eq!(queue.containment_candidate_count(), 1);
    assert_eq!(queue.admit(bounded(Some(10), 4, 8)), Ok((0, false)));
    queue.max_domains = 2;
    // The filter skips the impossible forward test; the remaining reverse
    // maintenance must still refuse counter overflow before retiring anything.
    queue.containment_checks = usize::MAX;
    assert_eq!(
        queue.admit(broad.clone()),
        Err("domain containment counter overflow")
    );
    assert_eq!(
        queue.by_owner[&(Phase::Apply, narrow.owner)].candidate_ids(),
        vec![0]
    );
    assert_eq!(queue.containment_maintenance_checks, 0);
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.exact.len(), 1);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.admit(narrow), Ok((0, false)));
    queue.containment_checks = 0;
    queue.containment_maintenance_checks = usize::MAX;
    assert_eq!(
        queue.admit(broad.clone()),
        Err("domain containment maintenance counter overflow")
    );
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].candidate_ids(),
        vec![0]
    );
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.exact.len(), 1);
    assert_maximal_index(&queue);
    queue.containment_maintenance_checks = 0;
    queue.containment_retired_candidates = usize::MAX;
    assert_eq!(
        queue.admit(broad),
        Err("domain containment retired-candidate counter overflow")
    );
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].candidate_ids(),
        vec![0]
    );
    assert_eq!(queue.containment_retired_candidates, usize::MAX);
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.exact.len(), 1);
    queue.containment_retired_candidates = 0;
    assert_maximal_index(&queue);
}

#[test]
fn finite_comparison_policy_keeps_the_historical_unpruned_index() {
    let mut queue = Queue::new(8, Some(100));
    assert_eq!(
        queue.containment_index_policy(),
        "historical_candidates_finite_cap"
    );
    let narrow = bounded(Some(11), 3, 9);
    queue.admit(narrow.clone()).unwrap();
    queue.admit(bounded(Some(11), 0, 12)).unwrap();
    assert_eq!(
        queue.by_owner[&(Phase::Apply, narrow.owner)].ids,
        vec![0, 1]
    );
    assert_eq!(queue.containment_maintenance_checks, 0);
    assert_eq!(queue.containment_retired_candidates, 0);
    assert_eq!(queue.containment_candidate_count(), 2);
    assert_eq!(queue.containment_checks, 1);
    assert_eq!(queue.admit(bounded(Some(10), 4, 8)), Ok((0, false)));
    assert_eq!(queue.containment_checks, 2);
}

#[test]
fn maximal_candidates_match_naive_fifo_for_overlapping_incomparable_and_expanding_boxes() {
    let mut queue = Queue::new(10_000, None);
    let mut baseline: Vec<Domain<2>> = Vec::new();
    let mut requests = Vec::new();
    for radius in 0..=12 {
        for center in [14, 28, 42] {
            for rank in [Some(0), Some(7), Some(u32::MAX), None] {
                for phase in [Phase::Apply, Phase::Route] {
                    let mut item = bounded(rank, center - radius, center + radius);
                    item.phase = phase;
                    requests.push(item.clone());
                    requests.push(item);
                }
            }
        }
    }
    for request in requests {
        let expected_new = !baseline.iter().any(|old| semantic_contains(old, &request));
        if expected_new {
            baseline.push(request.clone());
        }
        let (id, is_new) = queue.admit(request.clone()).unwrap();
        assert_eq!(is_new, expected_new);
        assert!(semantic_contains(&queue.domains[id], &request));
        if is_new {
            assert_eq!(id, baseline.len() - 1);
        }
        assert_eq!(queue.domains.len(), baseline.len());
        assert_maximal_index(&queue);
    }
    assert!(queue.domains.iter().map(Arc::as_ref).eq(baseline.iter()));
    assert_eq!(queue.exact.len(), baseline.len());
    assert_eq!(queue.next, 0);
    assert!(queue.containment_maintenance_checks > 0);
    assert!(
        queue
            .by_owner
            .values()
            .map(|bucket| bucket.candidate_ids().len())
            .sum::<usize>()
            < baseline.len()
    );
}
