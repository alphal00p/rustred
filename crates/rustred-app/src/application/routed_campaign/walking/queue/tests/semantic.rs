use super::*;

#[test]
fn equivalent_power_descriptions_reuse_pending_domain_without_rewriting_raw_keys() {
    let mut queue = Queue::new(8, None);
    let mut a = domain(None);
    a.owner = [true; 2];
    a.upper = vec![Some(1); 2];
    a.powers.max_positive_power = Some(4);
    let mut d = a.clone();
    d.powers = DomainPowerBounds {
        max_power_difference: Some(4),
        ..Default::default()
    };
    assert!(!a.contains(&d) && !d.contains(&a));
    assert_eq!(queue.admit(a.clone()), Ok((0, true)));
    assert_eq!(queue.admit(d), Ok((0, false)));
    assert_eq!(queue.containment_semantic_hits, 1);
    assert_eq!(queue.containment_summary_builds, 2);
    assert_eq!(queue.summaries.len(), 1);
    assert_eq!(queue.domains[0].as_ref(), &a);
    assert_eq!(queue.exact.len(), 1);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.admit(a), Ok((0, false)));
    assert_eq!(queue.containment_summary_builds, 2);
}

#[test]
fn implicit_rank_reuse_and_reverse_retirement_preserve_exact_ids() {
    let mut queue = Queue::new(8, None);
    let mut narrow = domain(Some(100));
    narrow.lower[0] = 2;
    narrow.upper = vec![Some(6), Some(4)];
    assert_eq!(queue.admit(narrow.clone()), Ok((0, true)));
    assert_eq!(queue.admit(domain(Some(5))), Ok((1, true)));
    assert_eq!(queue.containment_semantic_retirements, 1);
    assert_eq!(queue.containment_retired_candidates, 1);
    assert_eq!(queue.summaries.len(), 2);
    assert_eq!(queue.domains.len(), 2);
    assert_eq!(queue.next, 0);
    assert_eq!(queue.admit(narrow), Ok((0, false))); // raw exact ID retained
    let mut child = domain(None);
    child.upper = vec![Some(8), Some(4)];
    assert_eq!(queue.admit(child), Ok((1, false)));
    assert_eq!(queue.containment_semantic_hits, 1);
}

#[test]
fn capped_lane_retains_raw_geometry_and_does_not_construct_summaries() {
    let mut queue = Queue::new(8, Some(100));
    assert_eq!(queue.admit(domain(Some(5))), Ok((0, true)));
    let mut rank_label = domain(Some(100));
    rank_label.upper = vec![Some(8), Some(4)];
    assert_eq!(queue.admit(rank_label), Ok((1, true)));
    assert!(queue.summaries.is_empty());
    assert_eq!(queue.containment_summary_builds, 0);
    assert_eq!(queue.containment_semantic_hits, 0);
    assert_eq!(queue.containment_semantic_retirements, 0);
}

#[test]
fn malformed_summary_is_not_reused_even_through_an_orthant_shortcut() {
    let mut queue = Queue::new(8, None);
    queue.admit(domain(None)).unwrap();
    let mut invalid = domain(Some(0));
    invalid.lower[1] = 1; // rank-empty, but invalid D band must still fail
    invalid.powers.min_power_difference = Some(2);
    invalid.powers.max_power_difference = Some(1);
    assert_eq!(
        queue.admit(invalid),
        Err("inverted domain summary difference bounds")
    );
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.summaries.len(), 1);
    assert_eq!(queue.containment_summary_builds, 1);
    assert_eq!(queue.orthant_hits, 0);
    assert_eq!(queue.next, 0);
}

#[test]
fn semantic_counter_failure_never_publishes_or_retires_work() {
    let mut queue = Queue::new(8, None);
    let mut old = domain(Some(10));
    old.upper = vec![Some(4), Some(4)];
    queue.admit(old.clone()).unwrap();
    queue.containment_semantic_retirements = usize::MAX;
    assert_eq!(
        queue.admit(domain(Some(5))),
        Err("semantic containment retirement counter overflow")
    );
    assert_eq!(queue.by_owner[&(Phase::Apply, old.owner)].ids, vec![0]);
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.summaries.len(), 1);
    assert_eq!(queue.next, 0);
    queue.containment_semantic_retirements = 0;
    let mut equivalent = old.clone();
    equivalent.rank = None;
    queue.containment_semantic_hits = usize::MAX;
    assert_eq!(
        queue.admit(equivalent),
        Err("semantic containment hit counter overflow")
    );
    assert_eq!(queue.deduplicated, 0);
    assert_eq!(queue.domains.len(), 1);
    queue.containment_summary_builds = usize::MAX;
    assert_eq!(
        queue.admit(domain(Some(5))),
        Err("domain summary counter overflow")
    );
    assert_eq!(queue.admit(old), Ok((0, false))); // exact lookup needs no summary
}

fn point_mask(d: &Domain<2>) -> u16 {
    let mut bits = 0;
    for x in 0..=2 {
        for y in 0..=2 {
            let p = [x, y];
            if (0..2).any(|i| p[i] < d.lower[i] || d.upper[i].is_some_and(|u| p[i] > u)) {
                continue;
            }
            let a: u64 = (0..2).filter(|&i| d.owner[i]).map(|i| p[i] + 1).sum();
            let r: u64 = (0..2).filter(|&i| !d.owner[i]).map(|i| p[i]).sum();
            let delta = a as i64 - r as i64;
            if d.rank.is_none_or(|limit| r <= u64::from(limit))
                && d.powers.max_positive_power.is_none_or(|limit| a <= limit)
                && d.powers
                    .min_power_difference
                    .is_none_or(|limit| delta >= limit)
                && d.powers
                    .max_power_difference
                    .is_none_or(|limit| delta <= limit)
            {
                bits |= 1 << (3 * x + y);
            }
        }
    }
    bits
}

#[test]
fn semantic_queue_admission_matches_independent_finite_point_sets() {
    let mut queue = Queue::new(10_000, None);
    let mut baseline: Vec<(Domain<2>, u16)> = Vec::new();
    // Fully finite controls: membership above, not the production summary,
    // decides expected admission and containment. Include empty intersections.
    for cap in [2, 3, 4, 6] {
        for support in [[true, false], [false, true], [true, true], [false, false]] {
            for phase in [Phase::Apply, Phase::Route] {
                for lo in 0..=2 {
                    for rank in [Some(0), Some(2), Some(4), None] {
                        for dmin in [None, Some(-1), Some(1)] {
                            let d = Domain {
                                phase,
                                owner: support,
                                lower: vec![lo, 0],
                                upper: vec![Some(2); 2],
                                rank,
                                powers: DomainPowerBounds {
                                    max_positive_power: Some(cap),
                                    min_power_difference: dmin,
                                    max_power_difference: None,
                                },
                            };
                            let bits = point_mask(&d);
                            let expected_new = !baseline.iter().any(|(old, old_bits)| {
                                old.phase == phase && old.owner == support && bits & !old_bits == 0
                            });
                            if expected_new {
                                baseline.push((d.clone(), bits));
                            }
                            let (id, is_new) = queue.admit(d).unwrap();
                            assert_eq!(is_new, expected_new);
                            assert_eq!(queue.domains.len(), baseline.len());
                            assert_eq!(bits & !point_mask(&queue.domains[id]), 0);
                            assert_eq!(queue.domains[id].phase, phase);
                            assert_eq!(queue.domains[id].owner, support);
                            if is_new {
                                assert_eq!(id, baseline.len() - 1);
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(queue.next, 0);
    assert!(
        queue
            .domains
            .iter()
            .map(Arc::as_ref)
            .eq(baseline.iter().map(|(d, _)| d))
    );
    assert_eq!(queue.summaries.len(), queue.domains.len());
    assert!(queue.containment_semantic_hits > 0);
}
