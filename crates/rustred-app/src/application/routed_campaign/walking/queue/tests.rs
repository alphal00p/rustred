use super::*;

fn domain(rank: Option<u32>) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![0, 0],
        upper: vec![None, None],
        rank,
    }
}

#[test]
fn pending_inclusion_is_scheduling_reuse_not_completion() {
    let mut queue = Queue::new(3, 20);
    assert_eq!(queue.admit(domain(Some(10))), Ok((0, true)));
    let mut child = domain(Some(10));
    child.lower[0] = 3;
    assert_eq!(queue.admit(child), Ok((0, false)));
    assert_eq!(queue.next, 0);
    assert_eq!(queue.admit(domain(Some(11))), Ok((1, true)));
    assert_eq!(queue.admit(domain(None)), Ok((2, true)));
    assert_eq!(queue.admit(domain(Some(12))), Ok((2, false)));
    assert_eq!(queue.max_finite_rank, Some(11));
    assert_eq!(queue.unbounded_rank_domains, 1);
}

#[test]
fn literal_owner_and_unbounded_tail_are_not_approximated() {
    let mut queue = Queue::new(3, 20);
    let mut finite = domain(Some(10));
    finite.upper[0] = Some(u64::MAX);
    assert_eq!(queue.admit(finite), Ok((0, true)));
    assert_eq!(queue.admit(domain(Some(10))), Ok((1, true)));
    let mut other = domain(Some(10));
    other.owner = [false, true];
    assert_eq!(queue.admit(other), Ok((2, true)));
}

#[test]
fn indexed_reuse_survives_exhausted_scan_budget_without_scheduling_work() {
    let mut queue = Queue::new(1, 1);
    assert_eq!(queue.admit(domain(Some(10))), Ok((0, true)));
    assert_eq!(
        queue.admit(domain(Some(11))),
        Err("scheduled domain allowance")
    );
    assert_eq!(queue.containment_checks, 1);
    // Deliberate policy: the allowance limits general geometric comparisons,
    // not indexed proofs. Neither hit schedules work or raises admitted rank.
    assert_eq!(queue.admit(domain(Some(10))), Ok((0, false)));
    let mut child = domain(Some(9));
    child.lower[0] = 3;
    assert_eq!(queue.admit(child), Ok((0, false)));
    assert_eq!(
        queue.admit(domain(Some(12))),
        Err("domain containment check allowance")
    );
    assert_eq!(
        (queue.exact_hits, queue.orthant_hits, queue.deduplicated),
        (1, 1, 2)
    );
    assert_eq!(queue.containment_checks, 1);
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.exact.len(), 1);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.max_finite_rank, Some(10));
    assert_eq!(queue.unbounded_rank_domains, 0);
}

#[test]
fn route_and_apply_obligations_never_subsume_each_other() {
    let mut queue = Queue::new(3, 0);
    assert_eq!(queue.admit(domain(Some(11))), Ok((0, true)));
    let routed = Domain::route_cover([true, false], Some(11));
    assert_eq!(queue.admit(routed.clone()), Ok((1, true)));
    assert_eq!(queue.admit(routed), Ok((1, false)));
    let mut other = domain(Some(11));
    other.owner = [false, true];
    assert_eq!(queue.admit(other), Ok((2, true)));
    assert_eq!(queue.by_owner.len(), 3);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.containment_checks, 0);
}

#[test]
fn finite_maxima_are_not_infinity_in_either_index() {
    let mut queue = Queue::new(4, 100);
    let mut finite = domain(Some(u32::MAX));
    finite.upper[0] = Some(u64::MAX);
    assert_eq!(queue.admit(finite.clone()), Ok((0, true)));
    assert_eq!(queue.admit(domain(Some(u32::MAX))), Ok((1, true)));
    assert_eq!(queue.admit(domain(None)), Ok((2, true)));
    assert_eq!(queue.admit(finite), Ok((0, false)));
    let mut child = domain(None);
    child.lower[0] = u64::MAX;
    assert_eq!(queue.admit(child), Ok((2, false)));
    assert_eq!(queue.max_finite_rank, Some(u32::MAX));
    assert_eq!(queue.unbounded_rank_domains, 1);
    assert!(!rank_contains(Some(u32::MAX), None));
}

#[test]
fn dominant_orthant_may_reuse_different_valid_id_but_exact_id_stays_stable() {
    let mut queue = Queue::new(4, 100);
    let mut narrow = domain(Some(10));
    narrow.lower[0] = 5;
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    assert_eq!(queue.admit(domain(Some(10))), Ok((1, true)));
    let mut child = narrow.clone();
    child.lower[0] = 6;
    assert!(queue.domains[0].contains(&child));
    assert_eq!(queue.admit(child.clone()), Ok((1, false)));
    assert!(queue.domains[1].contains(&child));
    assert_eq!(queue.admit(narrow.clone()), Ok((0, false)));
    assert_eq!(queue.admit(domain(Some(11))), Ok((2, true)));
    assert_eq!(queue.admit(child), Ok((2, false)));
    assert_eq!(queue.admit(narrow), Ok((0, false)));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].orthant,
        Some(2)
    );
    assert_eq!(queue.next, 0);
}

#[test]
fn exact_index_shares_storage_and_never_caches_contained_request_keys() {
    let mut queue = Queue::new(2, 100);
    let mut narrow = domain(Some(10));
    narrow.lower[0] = 1;
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    let (indexed, &id) = queue.exact.get_key_value(&narrow).unwrap();
    assert_eq!(id, 0);
    assert!(Arc::ptr_eq(indexed, &queue.domains[0]));
    assert_eq!(Arc::strong_count(indexed), 2);
    assert_eq!(indexed.lower.as_ptr(), queue.domains[0].lower.as_ptr());
    // General fallback reuse also must not retain arbitrary observed keys.
    for lower in 2..12 {
        let mut child = narrow.clone();
        child.lower[0] = lower;
        assert_eq!(queue.admit(child), Ok((0, false)));
    }
    assert_eq!(queue.admit(domain(Some(10))), Ok((1, true)));
    for lower in 12..1000 {
        let mut child = narrow.clone();
        child.lower[0] = lower;
        assert_eq!(queue.admit(child), Ok((1, false)));
    }
    assert_eq!(queue.exact.len(), 2);
    assert_eq!(queue.domains.len(), 2);
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].ids,
        vec![0, 1]
    );
    assert_eq!(queue.containment_checks, 11);
}

#[test]
fn failed_domain_admission_does_not_publish_indices_or_rank_telemetry() {
    let mut queue = Queue::new(0, 100);
    assert_eq!(queue.admit(domain(None)), Err("scheduled domain allowance"));
    assert!(queue.domains.is_empty() && queue.exact.is_empty() && queue.by_owner.is_empty());
    assert_eq!(queue.max_finite_rank, None);
    assert_eq!(queue.unbounded_rank_domains, 0);
    queue.max_domains = 1;
    assert_eq!(queue.admit(domain(Some(10))), Ok((0, true)));
    assert_eq!(queue.admit(domain(None)), Err("scheduled domain allowance"));
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].orthant,
        Some(0)
    );
    let mut other = domain(None);
    other.owner = [false, true];
    assert_eq!(
        queue.admit(other.clone()),
        Err("scheduled domain allowance")
    );
    assert!(!queue.by_owner.contains_key(&(other.phase, other.owner)));
    assert_eq!(queue.exact.len(), 1);
    assert_eq!(queue.unbounded_rank_domains, 0);
    queue.max_domains = 3;
    assert_eq!(queue.admit(domain(None)), Ok((1, true)));
    assert_eq!(queue.admit(other), Ok((2, true)));
    assert_eq!(queue.max_finite_rank, Some(10));
    assert_eq!(queue.unbounded_rank_domains, 2);
}

#[test]
fn failed_comparison_admission_does_not_publish_dominant_orthant() {
    let mut queue = Queue::new(3, 0);
    let mut narrow = domain(Some(10));
    narrow.lower[0] = 1;
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    assert_eq!(
        queue.admit(domain(None)),
        Err("domain containment check allowance")
    );
    assert_eq!(queue.by_owner[&(Phase::Apply, [true, false])].orthant, None);
    assert_eq!(queue.exact.len(), 1);
    assert_eq!(queue.unbounded_rank_domains, 0);
    assert_eq!(queue.admit(narrow), Ok((0, false)));
    queue.max_checks = 1;
    assert_eq!(queue.admit(domain(None)), Ok((1, true)));
    assert_eq!(queue.unbounded_rank_domains, 1);
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].orthant,
        Some(1)
    );
}

#[test]
fn indexed_admission_matches_naive_admitted_fifo_for_mixed_domains() {
    let mut queue = Queue::new(10_000, usize::MAX);
    let mut baseline: Vec<Domain<2>> = Vec::new();
    let mut naive_checks = 0usize;
    let mut requests = Vec::new();
    // First exercise arbitrary boxes before adding any broad orthants.
    let mut seed = 17u64;
    for i in 0..800 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut candidate = domain(match (seed >> 16) % 5 {
            0 => None,
            1 => Some(u32::MAX),
            2 => Some(0),
            3 => Some(10),
            _ => Some(11),
        });
        candidate.phase = if i % 2 == 0 {
            Phase::Apply
        } else {
            Phase::Route
        };
        candidate.owner = match i % 3 {
            0 => [true, false],
            1 => [false, true],
            _ => [true, true],
        };
        for axis in 0..2 {
            let lower = (seed >> (axis * 8)) % 12;
            candidate.lower[axis] = lower;
            candidate.upper[axis] = match (seed >> (axis * 8 + 5)) % 4 {
                0 => None,
                1 => Some(u64::MAX),
                _ => Some(lower + 3),
            };
        }
        requests.push(candidate.clone());
        if i % 3 == 0 {
            requests.push(candidate);
        }
    }
    for rank in [Some(10), Some(11), Some(u32::MAX), None] {
        for phase in [Phase::Apply, Phase::Route] {
            for owner in [[true, false], [false, true], [true, true]] {
                let mut broad = domain(rank);
                broad.phase = phase;
                broad.owner = owner;
                requests.push(broad.clone());
                requests.push(broad.clone());
                broad.lower[0] = 30;
                requests.push(broad);
            }
        }
    }
    for request in requests {
        let prior = baseline.iter().position(|container| {
            if container.phase != request.phase || container.owner != request.owner {
                return false;
            }
            naive_checks += 1;
            container.contains(&request)
        });
        let expected_new = prior.is_none();
        if expected_new {
            baseline.push(request.clone());
        }
        let (id, is_new) = queue.admit(request.clone()).unwrap();
        assert_eq!(is_new, expected_new);
        assert!(queue.domains[id].contains(&request));
        if is_new {
            assert_eq!(id, baseline.len() - 1);
        }
        assert_eq!(queue.domains.len(), baseline.len());
    }
    assert!(queue.domains.iter().map(Arc::as_ref).eq(baseline.iter()));
    assert_eq!(queue.exact.len(), baseline.len());
    assert_eq!(queue.next, 0);
    assert!(queue.exact_hits > 0 && queue.orthant_hits > 0);
    assert!(queue.containment_checks < naive_checks);
    assert_eq!(
        queue.max_finite_rank,
        baseline.iter().filter_map(|d| d.rank).max()
    );
    assert_eq!(
        queue.unbounded_rank_domains,
        baseline.iter().filter(|d| d.rank.is_none()).count()
    );
}
