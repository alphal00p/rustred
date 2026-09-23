use super::*;
use rustred::solver::DomainPowerBounds;

fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn domain(phase: Phase, owner: [bool; 2], lower: [u64; 2], upper: [Option<u64>; 2]) -> Domain<2> {
    Domain {
        phase,
        owner,
        lower: lower.to_vec(),
        upper: upper.to_vec(),
        rank: None,
        powers: DomainPowerBounds::default(),
    }
}
fn seed(request: &OwnerDomainWalkRequest, domains: Vec<Domain<2>>) -> Walk<2> {
    let mut walk = Walk {
        buckets: BTreeMap::new(),
        initial: InitialOrthants::empty(),
        overlaps: BTreeMap::new(),
        budget: Budget::default(),
        initial_prepass_checks: 0,
        metrics: Metrics::default(),
        error: None,
        last_key: None,
    };
    for domain in domains {
        let key = (domain.phase, domain.owner);
        let bucket = walk
            .buckets
            .entry(key)
            .or_insert_with(|| Bucket::new(request).unwrap());
        let (_, added) = bucket.state.queue.admit(domain).unwrap();
        walk.budget.domains += usize::from(added);
    }
    for bucket in walk.buckets.values_mut() {
        bucket.finish_initial(request).unwrap();
    }
    walk
}
fn admission(domain: Domain<2>) -> Event<2> {
    Event::one(Effect::Admit {
        domain,
        successor: true,
        conditional: true,
    })
}

#[test]
fn initial_prepass_and_local_seeding_share_one_comparison_allowance() {
    for cap in [1, 2] {
        let mut request = request();
        request.max_containment_checks = Some(cap);
        let mut original = Queue::new(request.max_domains, request.max_containment_checks);
        for point in [0, 1] {
            original
                .admit(domain(
                    Phase::Apply,
                    [true, true],
                    [point, 0],
                    [Some(point), Some(0)],
                ))
                .unwrap();
        }
        assert_eq!(original.containment_checks, 1);
        let result = initialize(
            &original.domains,
            0,
            original.containment_checks,
            &request,
            &AtomicBool::new(false),
        );
        if cap == 1 {
            assert_eq!(result.err().unwrap(), "domain containment check allowance");
        } else {
            let (walk, handles) = result.unwrap();
            assert_eq!(handles.len(), 2);
            assert_eq!(walk.budget.checks, 2);
            let progress = report::progress(&walk, &request, json!({}), None);
            assert_eq!(progress["containment_checks"], 2);
            assert_eq!(progress["initial_prepass_containment_checks"], 1);
        }
    }
}

#[test]
fn discarded_initial_queries_still_consume_the_comparison_budget() {
    let mut request = request();
    request.max_containment_checks = Some(1);
    let mut original = Queue::new(request.max_domains, request.max_containment_checks);
    original
        .admit(domain(
            Phase::Apply,
            [true, true],
            [0, 0],
            [Some(2), Some(0)],
        ))
        .unwrap();
    assert_eq!(
        original
            .admit(domain(
                Phase::Apply,
                [true, true],
                [1, 0],
                [Some(1), Some(0)]
            ))
            .unwrap(),
        (0, false)
    );
    assert_eq!(original.containment_checks, 1);
    let (walk, _) = initialize(
        &original.domains,
        0,
        original.containment_checks,
        &request,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(walk.budget.checks, 1);
    assert_eq!(
        walk.buckets
            .values()
            .map(|b| b.state.queue.containment_checks)
            .sum::<usize>(),
        0
    );
}

#[test]
fn initial_comparison_remainder_is_shared_across_owner_buckets() {
    for cap in [3, 4] {
        let mut request = request();
        request.max_containment_checks = Some(cap);
        let mut original = Queue::new(request.max_domains, request.max_containment_checks);
        for phase in [Phase::Apply, Phase::Route] {
            for point in [0, 1] {
                original
                    .admit(domain(
                        phase,
                        [true, true],
                        [point, 0],
                        [Some(point), Some(0)],
                    ))
                    .unwrap();
            }
        }
        assert_eq!(original.containment_checks, 2);
        let result = initialize(
            &original.domains,
            0,
            original.containment_checks,
            &request,
            &AtomicBool::new(false),
        );
        if cap == 3 {
            assert_eq!(result.err().unwrap(), "domain containment check allowance");
        } else {
            let (walk, _) = result.unwrap();
            assert_eq!(walk.budget.checks, 4);
            assert_eq!(walk.buckets.len(), 2);
        }
    }
}

#[test]
fn owner_local_admission_keeps_diagnostics_with_source_and_geometry_at_destination() {
    let request = request();
    let source = (Phase::Apply, [true, false]);
    let destination = (Phase::Route, [false, true]);
    let initial = domain(source.0, source.1, [0, 0], [None, None]);
    let target = domain(destination.0, destination.1, [2, 3], [Some(4), Some(5)]);
    let mut walk = seed(&request, vec![initial]);
    let events = vec![
        admission(target.clone()),
        Event::one(Effect::Frontier {
            value: json!({"kind":"source-control"}),
            successor: false,
            conditional: false,
        }),
    ];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    delivery::deliver(
        &mut walk,
        vec![(source, events)],
        &request,
        &AtomicBool::new(false),
        Some(&pool),
    )
    .unwrap();
    assert_eq!(*walk.buckets[&destination].state.queue.domains[0], target);
    assert_eq!(
        walk.buckets[&source].state.details,
        vec![json!({"kind":"source-control"})]
    );
    assert!(walk.buckets[&destination].state.details.is_empty());
    assert_eq!(walk.buckets[&source].state.successors, 1);
    assert_eq!(walk.buckets[&source].state.conditional, 1);
    assert_eq!(walk.metrics.delivered_cross_owner_requests, 1);
    assert_eq!(walk.budget.events, 2);
    assert_eq!(walk.budget.frontiers, 1);
    assert_eq!(walk.budget.domains, 2);
}

#[test]
fn global_domain_cap_still_accepts_cross_owner_reuse_then_refuses_genuine_new_work() {
    let mut request = request();
    request.max_domains = 2;
    let source = (Phase::Apply, [true, false]);
    let destination = (Phase::Apply, [false, true]);
    let a = domain(source.0, source.1, [0, 0], [None, None]);
    let b = domain(destination.0, destination.1, [0, 0], [None, None]);
    let mut walk = seed(&request, vec![a, b]);
    let subset = domain(destination.0, destination.1, [2, 3], [Some(4), Some(5)]);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    delivery::deliver(
        &mut walk,
        vec![(source, vec![admission(subset)])],
        &request,
        &AtomicBool::new(false),
        Some(&pool),
    )
    .unwrap();
    assert_eq!(walk.budget.domains, 2);
    assert_eq!(walk.buckets[&destination].state.queue.deduplicated, 1);
    assert_eq!(
        walk.buckets[&destination].state.queue.containment_limit(),
        None
    );
    let new = domain(Phase::Route, [true, true], [0, 0], [Some(0), Some(0)]);
    assert_eq!(
        delivery::deliver(
            &mut walk,
            vec![(source, vec![admission(new)])],
            &request,
            &AtomicBool::new(false),
            Some(&pool)
        ),
        Err("scheduled domain allowance".into())
    );
    assert_eq!(walk.budget.domains, 2);
    assert_eq!(walk.budget.events, 2);
}

#[test]
fn counted_reuse_honors_global_event_cap_across_source_buckets() {
    for cap in 1..=5 {
        let mut request = request();
        request.max_events = cap;
        let a = (Phase::Apply, [true, false]);
        let b = (Phase::Apply, [false, true]);
        let mut walk = seed(
            &request,
            vec![
                domain(a.0, a.1, [0, 0], [None, None]),
                domain(b.0, b.1, [0, 0], [None, None]),
            ],
        );
        let chunks = vec![
            (
                a,
                vec![Event {
                    count: 2,
                    effect: Effect::KnownReuse {
                        successor: true,
                        conditional: false,
                    },
                }],
            ),
            (
                b,
                vec![Event {
                    count: 3,
                    effect: Effect::KnownReuse {
                        successor: true,
                        conditional: true,
                    },
                }],
            ),
        ];
        let result = delivery::deliver(&mut walk, chunks, &request, &AtomicBool::new(false), None);
        assert_eq!(result.is_ok(), cap == 5);
        assert_eq!(walk.budget.events, cap);
        assert_eq!(
            walk.buckets
                .values()
                .map(|b| b.state.queue.deduplicated)
                .sum::<usize>(),
            cap
        );
        assert_eq!(walk.buckets[&a].state.conditional, 0);
        assert_eq!(walk.buckets[&b].state.conditional, cap.saturating_sub(2));
    }
}

#[test]
fn cancellation_before_delivery_preserves_every_pending_domain() {
    let request = request();
    let source = (Phase::Apply, [true, false]);
    let mut walk = seed(
        &request,
        vec![domain(source.0, source.1, [0, 0], [None, None])],
    );
    let target = domain(Phase::Route, [false, true], [0, 0], [None, None]);
    assert_eq!(
        delivery::deliver(
            &mut walk,
            vec![(source, vec![admission(target)])],
            &request,
            &AtomicBool::new(true),
            None
        ),
        Err("cancelled".into())
    );
    assert_eq!(walk.budget.domains, 1);
    assert_eq!(walk.budget.events, 0);
    assert_eq!(walk.buckets[&source].state.queue.next, 0);
}

#[test]
fn parallel_destinations_preserve_each_destination_source_order_and_representatives() {
    let request = request();
    let a = (Phase::Apply, [true, false]);
    let b = (Phase::Apply, [false, true]);
    let initial = vec![
        domain(a.0, a.1, [0, 0], [Some(0), Some(0)]),
        domain(b.0, b.1, [0, 0], [Some(0), Some(0)]),
    ];
    let chunks = || {
        vec![
            (
                a,
                vec![
                    admission(domain(Phase::Route, a.1, [1, 0], [Some(2), Some(3)])),
                    admission(domain(Phase::Route, b.1, [3, 0], [Some(4), Some(3)])),
                ],
            ),
            (
                b,
                vec![
                    admission(domain(Phase::Route, a.1, [1, 1], [Some(2), Some(2)])),
                    admission(domain(Phase::Route, b.1, [3, 1], [Some(4), Some(2)])),
                ],
            ),
        ]
    };
    let mut serial = seed(&request, initial.clone());
    let mut parallel = seed(&request, initial);
    delivery::deliver(
        &mut serial,
        chunks(),
        &request,
        &AtomicBool::new(false),
        None,
    )
    .unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    delivery::deliver(
        &mut parallel,
        chunks(),
        &request,
        &AtomicBool::new(false),
        Some(&pool),
    )
    .unwrap();
    assert_eq!(serial.budget.domains, parallel.budget.domains);
    for (key, a) in &serial.buckets {
        let b = &parallel.buckets[key];
        assert_eq!(a.state.queue.domains, b.state.queue.domains);
        assert_eq!(a.state.queue.deduplicated, b.state.queue.deduplicated);
        assert_eq!(
            a.state.queue.containment_retired_candidates,
            b.state.queue.containment_retired_candidates
        );
        assert_eq!(a.state.successors, b.state.successors);
    }
}

#[test]
fn finite_comparison_policy_is_preserved_by_global_budget_fallback() {
    let mut request = request();
    request.max_containment_checks = Some(1);
    let source = (Phase::Apply, [true, false]);
    let mut walk = seed(
        &request,
        vec![domain(source.0, source.1, [0, 0], [Some(0), Some(0)])],
    );
    let target = domain(source.0, source.1, [1, 0], [Some(1), Some(0)]);
    delivery::deliver(
        &mut walk,
        vec![(source, vec![admission(target.clone())])],
        &request,
        &AtomicBool::new(false),
        None,
    )
    .unwrap();
    assert_eq!(walk.budget.checks, 1);
    // Exact reuse is legal even after the aggregate comparison allowance ends.
    delivery::deliver(
        &mut walk,
        vec![(source, vec![admission(target)])],
        &request,
        &AtomicBool::new(false),
        None,
    )
    .unwrap();
    let fresh = domain(source.0, source.1, [2, 0], [Some(2), Some(0)]);
    assert_eq!(
        delivery::deliver(
            &mut walk,
            vec![(source, vec![admission(fresh)])],
            &request,
            &AtomicBool::new(false),
            None
        ),
        Err("domain containment check allowance".into())
    );
    assert_eq!(walk.budget.checks, 1);
    assert_eq!(
        walk.buckets[&source].state.queue.containment_limit(),
        Some(1)
    );
}

#[test]
fn delivery_counter_overflow_is_an_explicit_refusal() {
    let request = request();
    let source = (Phase::Apply, [true, false]);
    let mut walk = seed(
        &request,
        vec![domain(source.0, source.1, [0, 0], [None, None])],
    );
    walk.metrics.serial_budget_batches = usize::MAX;
    assert_eq!(
        delivery::deliver(
            &mut walk,
            vec![(source, vec![Event::one(Effect::Count)])],
            &request,
            &AtomicBool::new(false),
            None
        ),
        Err("serial batch counter overflow".into())
    );
    assert_eq!(walk.budget.events, 0);
}

#[test]
fn temporary_admission_budgets_reject_below_length_and_comparison_lane_switches() {
    let original = domain(Phase::Apply, [true, false], [0, 0], [Some(3), Some(3)]);
    for comparison_limit in [None, Some(3)] {
        let mut queue = Queue::new(3, comparison_limit);
        assert_eq!(queue.admit(original.clone()), Ok((0, true)));
        let builds = queue.containment_summary_builds;
        assert_eq!(
            queue.admit_with_budget(original.clone(), 0, comparison_limit),
            Err("invalid narrowed admission budget")
        );
        let other_lane = if comparison_limit.is_none() {
            Some(0)
        } else {
            None
        };
        assert_eq!(
            queue.admit_with_budget(original.clone(), 1, other_lane),
            Err("invalid narrowed admission budget")
        );
        assert_eq!(queue.domains.len(), 1);
        assert_eq!(queue.exact_hits, 0);
        assert_eq!(queue.containment_summary_builds, builds);
        assert_eq!(queue.containment_limit(), comparison_limit);
        // Exact reuse is still legal at the exact current domain allowance.
        assert_eq!(
            queue.admit_with_budget(original.clone(), 1, comparison_limit),
            Ok((0, false))
        );
        assert_eq!(queue.containment_limit(), comparison_limit);
    }
}

#[test]
fn temporary_domain_budget_accepts_nonexact_semantic_reuse_and_restores_after_refusal() {
    let original = domain(Phase::Apply, [true, false], [0, 0], [Some(3), Some(3)]);
    let subset = domain(Phase::Apply, [true, false], [1, 1], [Some(2), Some(2)]);
    let novel = domain(Phase::Apply, [true, false], [4, 0], [Some(4), Some(0)]);
    let mut queue = Queue::new(3, None);
    assert_eq!(queue.admit(original), Ok((0, true)));
    assert_eq!(queue.admit_with_budget(subset, 1, None), Ok((0, false)));
    assert_eq!(queue.exact_hits, 0);
    assert_eq!(queue.orthant_hits, 0);
    assert_eq!(queue.containment_limit(), None);
    assert_eq!(
        queue.containment_index_policy(),
        "maximal_candidates_semantic_unlimited"
    );
    assert_eq!(
        queue.admit_with_budget(novel.clone(), 1, None),
        Err("scheduled domain allowance")
    );
    assert_eq!(queue.domains.len(), 1);
    assert_eq!(queue.containment_limit(), None);
    // This would fail if the temporary domain limit leaked from the refusal.
    assert_eq!(queue.admit(novel), Ok((1, true)));
    assert_eq!(queue.containment_candidate_count(), 2);
}

#[test]
fn temporary_finite_comparison_budget_is_restored_after_a_refused_lookup() {
    let original = domain(Phase::Apply, [true, false], [0, 0], [Some(0), Some(0)]);
    let novel = domain(Phase::Apply, [true, false], [1, 0], [Some(1), Some(0)]);
    let mut queue = Queue::new(3, Some(3));
    queue.admit(original.clone()).unwrap();
    assert_eq!(
        queue.admit_with_budget(novel.clone(), 3, Some(0)),
        Err("domain containment check allowance")
    );
    assert_eq!(queue.containment_checks, 0);
    assert_eq!(queue.containment_limit(), Some(3));
    assert_eq!(queue.admit(novel), Ok((1, true)));
    assert_eq!(queue.containment_checks, 1);
    assert_eq!(queue.containment_summary_builds, 0);
    assert_eq!(
        queue.containment_index_policy(),
        "historical_candidates_finite_cap"
    );
    assert_eq!(
        queue.admit_with_budget(original, 2, Some(0)),
        Err("invalid narrowed admission budget")
    );
    assert_eq!(queue.containment_limit(), Some(3));
}

#[test]
fn numeric_event_limits_preserve_ordered_wrapper_prefix_and_frontier_semantics() {
    for cap in 0..=6 {
        for kind in 0..3 {
            let mut request = request();
            request.max_events = cap;
            let event = || Event {
                count: 5,
                effect: match kind {
                    0 => Effect::Count,
                    1 => Effect::KnownReuse {
                        successor: true,
                        conditional: true,
                    },
                    _ => Effect::PreAdmittedOrthantReuse {
                        successor: true,
                        conditional: false,
                    },
                },
            };
            let mut ordered = State::new(Queue::<2>::new(5, None), 0, None);
            let mut numeric = State::new(Queue::<2>::new(5, None), 0, None);
            assert_eq!(
                ordered.accept(event(), &request),
                numeric.accept_with_limits(event(), cap, request.max_frontiers)
            );
            assert_eq!(ordered.events, numeric.events);
            assert_eq!(ordered.successors, numeric.successors);
            assert_eq!(ordered.conditional, numeric.conditional);
            assert_eq!(ordered.queue.deduplicated, numeric.queue.deduplicated);
            assert_eq!(ordered.job_local_reuse_hits, numeric.job_local_reuse_hits);
            assert_eq!(
                ordered.pre_admitted_orthant_hits,
                numeric.pre_admitted_orthant_hits
            );
        }
    }
    for cap in 0..=2 {
        let mut request = request();
        request.max_frontiers = cap;
        let event = || {
            Event::one(Effect::Frontier {
                value: json!({"kind":"numeric-limit-control"}),
                successor: true,
                conditional: false,
            })
        };
        let mut ordered = State::new(Queue::<2>::new(5, None), 0, None);
        let mut numeric = State::new(Queue::<2>::new(5, None), 0, None);
        for _ in 0..3 {
            assert_eq!(
                ordered.accept(event(), &request),
                numeric.accept_with_limits(event(), request.max_events, cap)
            );
            assert_eq!(ordered.events, numeric.events);
            assert_eq!(ordered.frontiers, numeric.frontiers);
            assert_eq!(ordered.details, numeric.details);
        }
    }
}
