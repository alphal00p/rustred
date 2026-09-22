use super::*;

#[test]
fn power_predicates_participate_in_identity_and_dominance_without_dropping_work() {
    let mut queue = Queue::new(8, None);
    let mut bounded = domain(Some(4));
    bounded.upper = vec![Some(9), Some(4)];
    bounded.powers = DomainPowerBounds {
        max_positive_power: Some(5),
        min_power_difference: Some(1),
        max_power_difference: Some(3),
    };
    assert_eq!(queue.admit(bounded.clone()), Ok((0, true)));
    let mut wider = bounded.clone();
    wider.powers.min_power_difference = Some(0);
    assert_eq!(queue.admit(wider.clone()), Ok((1, true)));
    assert_eq!(queue.containment_retired_candidates, 1);
    assert_eq!(queue.domains.len(), 2);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.admit(bounded.clone()), Ok((0, false)));
    bounded.powers.max_power_difference = Some(2);
    assert_eq!(queue.admit(bounded), Ok((1, false)));
    wider.powers.max_power_difference = Some(4);
    assert_eq!(queue.admit(wider), Ok((2, true)));
}

mod aggregate;
mod linear_semantic;
mod maximal_candidates;
mod prepared;
mod replay;
mod semantic;

fn semantic_contains<const N: usize>(container: &Domain<N>, candidate: &Domain<N>) -> bool {
    let summary = |d: &Domain<N>| {
        DomainPowerSummary::try_new(d.owner, &d.lower, &d.upper, d.rank, d.powers).unwrap()
    };
    container.phase == candidate.phase
        && container.owner == candidate.owner
        && summary(container).contains(&summary(candidate))
}

fn domain(rank: Option<u32>) -> Domain<2> {
    Domain {
        powers: Default::default(),
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![0, 0],
        upper: vec![None, None],
        rank,
    }
}

#[test]
fn unlimited_comparisons_pass_former_default_and_overflow_without_admitting() {
    let mut queue = Queue::new(3, None);
    assert_eq!(queue.containment_limit(), None);
    let mut narrow = domain(Some(10));
    narrow.lower[0] = 1;
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    let mut child = narrow.clone();
    child.lower[0] = 2;
    queue.containment_checks = 10_000_000;
    assert_eq!(queue.admit(child.clone()), Ok((0, false)));
    assert_eq!(queue.containment_checks, 10_000_001);
    queue.containment_checks = usize::MAX - 1;
    assert_eq!(queue.admit(child.clone()), Ok((0, false)));
    assert_eq!(queue.containment_checks, usize::MAX);
    let before = (
        queue.domains.len(),
        queue.exact.len(),
        queue.deduplicated,
        queue.next,
        queue.max_finite_rank,
        queue.unbounded_rank_domains,
    );
    assert_eq!(
        queue.admit(child),
        Err("domain containment counter overflow")
    );
    assert_eq!(queue.containment_checks, usize::MAX);
    assert_eq!(
        (
            queue.domains.len(),
            queue.exact.len(),
            queue.deduplicated,
            queue.next,
            queue.max_finite_rank,
            queue.unbounded_rank_domains
        ),
        before
    );
    // Exact lookup needs no scan and still works after the counter is full.
    assert_eq!(queue.admit(narrow), Ok((0, false)));
    assert_eq!(queue.containment_checks, usize::MAX);
}

#[test]
fn finite_comparison_prefix_is_unchanged_and_orthant_lookup_avoids_overflow() {
    let mut finite = Queue::new(3, Some(2));
    let mut unlimited = Queue::new(3, None);
    let mut narrow = domain(Some(10));
    narrow.lower[0] = 1;
    for queue in [&mut finite, &mut unlimited] {
        assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    }
    for value in [2, 3] {
        let mut child = narrow.clone();
        child.lower[0] = value;
        assert_eq!(finite.admit(child.clone()), unlimited.admit(child));
    }
    let mut child = narrow;
    child.lower[0] = 4;
    assert_eq!(
        finite.admit(child.clone()),
        Err("domain containment check allowance")
    );
    assert_eq!(unlimited.admit(child), Ok((0, false)));
    assert_eq!((finite.containment_checks, finite.deduplicated), (2, 2));
    assert_eq!(
        (unlimited.containment_checks, unlimited.deduplicated),
        (3, 3)
    );
    let mut indexed = Queue::new(1, None);
    indexed.admit(domain(Some(10))).unwrap();
    indexed.containment_checks = usize::MAX;
    let mut child = domain(Some(9));
    child.lower[0] = 4;
    assert_eq!(indexed.admit(child), Ok((0, false)));
    assert_eq!(
        (indexed.containment_checks, indexed.orthant_hits),
        (usize::MAX, 1)
    );
}

#[test]
fn pending_inclusion_is_scheduling_reuse_not_completion() {
    let mut queue = Queue::new(3, Some(20));
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
    let mut queue = Queue::new(3, Some(20));
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
    let mut queue = Queue::new(1, Some(1));
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
    let mut queue = Queue::new(3, Some(0));
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
    let mut queue = Queue::new(4, Some(100));
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
    let mut queue = Queue::new(4, Some(100));
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
    let mut queue = Queue::new(2, Some(100));
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
    let mut queue = Queue::new(0, Some(100));
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
    let mut queue = Queue::new(3, Some(0));
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
    queue.max_checks = Some(1);
    assert_eq!(queue.admit(domain(None)), Ok((1, true)));
    assert_eq!(queue.unbounded_rank_domains, 1);
    assert_eq!(
        queue.by_owner[&(Phase::Apply, [true, false])].orthant,
        Some(1)
    );
}

#[test]
fn indexed_admission_matches_naive_semantic_fifo_for_mixed_domains() {
    let mut queue = Queue::new(10_000, None);
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
            semantic_contains(container, &request)
        });
        let expected_new = prior.is_none();
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
    }
    assert!(queue.domains.iter().map(Arc::as_ref).eq(baseline.iter()));
    assert_eq!(queue.exact.len(), baseline.len());
    assert_eq!(queue.next, 0);
    assert!(queue.exact_hits > 0 && queue.orthant_hits > 0);
    // Query comparisons and optional reverse index maintenance are both
    // accounted. This fixture compares the former with the naive search.
    assert!(queue.containment_checks - queue.containment_maintenance_checks < naive_checks);
    assert!(queue.containment_maintenance_checks > 0);
    assert_eq!(
        queue.max_finite_rank,
        baseline.iter().filter_map(|d| d.rank).max()
    );
    assert_eq!(
        queue.unbounded_rank_domains,
        baseline.iter().filter(|d| d.rank.is_none()).count()
    );
}
